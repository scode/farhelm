//! Close registered fixed popovers when a causal report invalidates their row.
//!
//! The open slots and their focus/measurement lifetimes stay with the existing
//! components. This owner reads those slots without subscribing to them, then
//! invokes their idempotent closers at the reporting caller's own boundary.
//! It keeps no movement history: an opening after a report measures the new
//! layout and must never receive a replay of that earlier invalidation.
//!
//! New content reports additionally capture a weak row-local opening
//! binding. That admission distinguishes retained root slots from mounted
//! measurement lifetimes; it does not narrow any existing legacy dispatch.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::{Rc, Weak};

use dioxus::prelude::*;

/// Independent row orders; a host permutation need not move any session row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AnchorRegion {
    Hosts,
    Sessions,
    AppBar,
    Templates,
}

/// Region membership and the one existing proactive disclosure policy.
///
/// Direct host Details commands dismiss row action panels before rendered-size
/// reporting, including repeated reveals that do not move anything. Bell lists
/// instead retain their existing later layout-report boundary. This opt-in is
/// declared at registration so producers never enumerate popup kinds.
#[derive(Clone, Copy)]
pub(crate) struct Interest {
    pub(crate) region: AnchorRegion,
    pub(crate) direct_host_disclosure: bool,
}

impl Interest {
    /// Ordinary movement interest; proactive host disclosure is opt-in.
    pub(crate) fn new(region: AnchorRegion) -> Self {
        Self {
            region,
            direct_host_disclosure: false,
        }
    }
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
    direct_host_disclosure: bool,
    current_row: Box<dyn Fn() -> Option<String>>,
    close: Box<dyn Fn()>,
    // Enrolled openings bind here. Legacy dispatch deliberately ignores
    // admission: its historical closed-slot and recovery policies still apply.
    anchor: RefCell<Weak<Anchor>>,
}

impl Entry {
    /// End the binding borrow before any user-supplied live reader runs.
    fn has_anchor(&self, anchor: &Weak<Anchor>) -> bool {
        Weak::ptr_eq(&self.anchor.borrow(), anchor)
    }
}

/// A row's current measurement lifetime, without handles or geometry.
/// The row owns the strong lease; registry bindings and captured tickets are
/// weak so a retained root open slot cannot keep an unmounted row alive.
struct Anchor {
    row: String,
    ready: Box<dyn Fn() -> bool>,
}

/// Retire just this row's binding when its measurement scope disappears.
/// Replacing a binding or opening revocation also invalidates existing tickets.
pub(crate) struct AnchorLease {
    registration: Registration,
    anchor: Rc<Anchor>,
}

/// A captured opening, never an instruction to find a later open slot.
/// Identity, weak lifetime and live row/listing checks all must still agree at
/// delivery. Pending and fallback measurements belong to the same opening.
pub(crate) struct OpeningTicket {
    owner: Weak<RefCell<Registry>>,
    registration: u64,
    anchor: Weak<Anchor>,
}

/// A bounded regional snapshot of the openings admitted at a cause boundary.
/// Producers may deliver immediately or later; a fresh opening receives no
/// replay, even when it uses the same row as a captured opening.
pub(crate) struct MovementTicket(Vec<OpeningTicket>);

/// A mount's registration; clones share one lease and unregister together.
///
/// The weak reference avoids extending the application's lifetime. Explicit
/// scope cleanup removes callbacks before their component signals are retired;
/// dropping the last lease also removes plain, non-component registrations.
#[derive(Clone)]
pub(crate) struct Registration(Rc<Lease>);

impl PartialEq for Registration {
    /// Component props compare a stable mount capability, not mutable state.
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

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
        interest: Interest,
        current_row: impl Fn() -> Option<String> + 'static,
        close: impl Fn() + 'static,
    ) -> Registration {
        let mut registry = self.0.borrow_mut();
        let id = registry.next_id;
        registry.next_id += 1;
        registry.entries.push((
            id,
            Rc::new(Entry {
                region: interest.region,
                direct_host_disclosure: interest.direct_host_disclosure,
                current_row: Box::new(current_row),
                close: Box::new(close),
                anchor: RefCell::new(Weak::new()),
            }),
        ));
        Registration(Rc::new(Lease {
            owner: Rc::downgrade(&self.0),
            id,
        }))
    }

    /// Close open registrations on selected rows, at the caller's boundary.
    pub(crate) fn close_affected(&self, moved: &MovedRows) {
        self.dispatch(|_, entry| {
            entry.region == moved.region
                && (entry.current_row)().is_some_and(|row| moved.rows.contains(&row))
        });
    }

    /// Capture current openings in a region with an actual row-local lease.
    /// Legacy reports remain independent of this optional admission. Caller
    /// readiness excludes Err before teardown; the scoped lease excludes Ok
    /// before a recovery mount starts its measurement.
    pub(crate) fn capture_region(&self, region: AnchorRegion) -> MovementTicket {
        self.capture_selected(|entry, _| entry.region == region)
    }

    /// Capture only affected current rows through the same scoped admission
    /// as regional reports. The caller owns displayed-order selection.
    pub(crate) fn capture_affected(&self, moved: &MovedRows) -> MovementTicket {
        self.capture_selected(|entry, row| entry.region == moved.region && moved.rows.contains(row))
    }

    /// Snapshot weak opening identities without admitting unrelated regions.
    fn capture_selected(&self, selected: impl Fn(&Entry, &str) -> bool) -> MovementTicket {
        MovementTicket(
            self.snapshot()
                .into_iter()
                .filter_map(|(id, entry)| {
                    let anchor = entry.anchor.borrow().upgrade()?;
                    if selected(&entry, &anchor.row) {
                        self.capture_entry(id, &entry, &anchor.row)
                    } else {
                        None
                    }
                })
                .collect(),
        )
    }

    /// Validate readers outside registry borrows, then retain weak identities.
    /// A row-filtered capture prevents one bell's future width producer from
    /// capturing another row that shares its parent registration.
    fn capture_entry(&self, id: u64, entry: &Entry, row: &str) -> Option<OpeningTicket> {
        if !self.contains(id) {
            return None;
        }
        let anchor = entry.anchor.borrow().upgrade()?;
        if row != anchor.row.as_str()
            || (entry.current_row)().as_deref() != Some(anchor.row.as_str())
            || !entry.has_anchor(&Rc::downgrade(&anchor))
            || !(anchor.ready)()
            || !self.contains(id)
            || !entry.has_anchor(&Rc::downgrade(&anchor))
            || (entry.current_row)().as_deref() != Some(anchor.row.as_str())
            || !entry.has_anchor(&Rc::downgrade(&anchor))
        {
            return None;
        }
        Some(OpeningTicket {
            owner: Rc::downgrade(&self.0),
            registration: id,
            anchor: Rc::downgrade(&anchor),
        })
    }

    /// Invalidate whole row regions without writing already-closed slots.
    ///
    /// The broad sidebar layout effect historically checked each slot first;
    /// opening a slot must not itself become a dependency of that effect.
    pub(crate) fn close_open_in(&self, regions: &[AnchorRegion]) {
        self.dispatch(|_, entry| {
            regions.contains(&entry.region) && (entry.current_row)().is_some()
        });
    }

    /// Deliver an unconditional regional clear, including to closed slots.
    ///
    /// Compact changes and a local Delete that invalidates its sole anchor
    /// write every session-popover slot even when one is already None. Those
    /// writes can schedule effects, so they are distinct from close_affected.
    pub(crate) fn close_registered(&self, region: AnchorRegion) {
        self.dispatch(|_, entry| entry.region == region);
    }

    /// Preserve the direct Details command's unconditional host-then-session
    /// clear, independently of the broad layout effect's registration order.
    ///
    /// Actual rendered movement reaches all affected popovers through ordinary
    /// reports. This earlier command deliberately reaches only opted-in action
    /// panels, including their closed slots, and does not broaden bell timing.
    pub(crate) fn close_for_host_disclosure(&self) {
        for region in [AnchorRegion::Hosts, AnchorRegion::Sessions] {
            self.dispatch(|_, entry| entry.region == region && entry.direct_host_disclosure);
        }
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
    fn dispatch(&self, selected: impl Fn(u64, &Entry) -> bool) {
        for (id, entry) in self.snapshot() {
            if self.contains(id) && selected(id, &entry) && self.contains(id) {
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
    /// Revoke old admission synchronously before publishing a fresh root open.
    /// Bell measurement still begins in its existing effect; this metadata
    /// boundary prevents a deferred old ticket from reaching that new open.
    pub(crate) fn forget_anchor(&self) {
        if let Some(entry) = self.entry() {
            *entry.anchor.borrow_mut() = Weak::new();
        }
    }

    /// Bind at an existing measurement start, only for the live row.
    /// A late callback for A must not replace B's newer binding. `ready` is an
    /// untracked successful-listing check; scope and opening identity come from
    /// this lease, without a second handle, placement or generation policy.
    pub(crate) fn bind_anchor(
        &self,
        row: String,
        ready: impl Fn() -> bool + 'static,
    ) -> Option<AnchorLease> {
        let entry = self.entry()?;
        if (entry.current_row)().as_deref() != Some(row.as_str()) || self.entry().is_none() {
            return None;
        }
        let anchor = Rc::new(Anchor {
            row,
            ready: Box::new(ready),
        });
        *entry.anchor.borrow_mut() = Rc::downgrade(&anchor);
        Some(AnchorLease {
            registration: self.clone(),
            anchor,
        })
    }

    /// Clone the callback record without holding its registry borrow.
    fn entry(&self) -> Option<Rc<Entry>> {
        let owner = self.0.owner.upgrade()?;
        owner
            .borrow()
            .entries
            .iter()
            .find(|(id, _)| *id == self.0.id)
            .map(|(_, entry)| entry.clone())
    }

    /// Continue this registration's existing epoch effect through the owner.
    ///
    /// The caller keeps its own dependencies, initial execution and admission
    /// rule (bar opening epoch versus template's unconditional epoch effect).
    /// Only this open registration is selected; no global menu policy is added.
    pub(crate) fn close_if_open(&self) {
        if let Some(owner) = self.0.owner.upgrade() {
            MovementOwner(owner)
                .dispatch(|id, entry| id == self.0.id && (entry.current_row)().is_some());
        }
    }

    /// Retire this mount's callbacks before its signal storage disappears.
    fn unregister(&self) {
        self.0.unregister();
    }
}

impl OpeningTicket {
    /// Invoke the original closer only while this exact opening remains live.
    /// Recheck after the admission callback because it can retire a component;
    /// no closer is called while registry or binding storage is borrowed.
    pub(crate) fn close_if_current(self) {
        let Some(owner) = self.owner.upgrade().map(MovementOwner) else {
            return;
        };
        let Some(anchor) = self.anchor.upgrade() else {
            return;
        };
        owner.dispatch(|id, entry| {
            id == self.registration
                && entry.has_anchor(&self.anchor)
                && (entry.current_row)().as_deref() == Some(anchor.row.as_str())
                && entry.has_anchor(&self.anchor)
                && (anchor.ready)()
                && entry.has_anchor(&self.anchor)
                && (entry.current_row)().as_deref() == Some(anchor.row.as_str())
                && entry.has_anchor(&self.anchor)
        });
    }
}

impl MovementTicket {
    /// Preserve regional registration order while dropping superseded openings.
    pub(crate) fn close_if_current(self) {
        for ticket in self.0 {
            ticket.close_if_current();
        }
    }
}

impl Drop for AnchorLease {
    fn drop(&mut self) {
        if let Some(entry) = self.registration.entry() {
            let current = Weak::ptr_eq(&entry.anchor.borrow(), &Rc::downgrade(&self.anchor));
            if current {
                *entry.anchor.borrow_mut() = Weak::new();
            }
        }
    }
}

/// Keep one row-local lease across renders, retiring it before row signals.
/// The Copy callback preserves the existing synchronous menu open handlers and
/// delayed bell measurement effect without adding a reactive dependency.
pub(crate) fn use_anchor_binding(
    registration: Registration,
    row: String,
    ready: Callback<(), bool>,
) -> Callback<()> {
    let lease = use_hook(|| Rc::new(RefCell::new(None::<AnchorLease>)));
    let on_drop = lease.clone();
    use_drop(move || {
        on_drop.borrow_mut().take();
    });
    use_callback(move |()| {
        *lease.borrow_mut() = registration.bind_anchor(row.clone(), move || ready.call(()));
    })
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
    interest: Interest,
    current_row: impl Fn() -> Option<String> + 'static,
    close: impl Fn() + 'static,
) -> Registration {
    let registration = use_hook(move || owner.register(interest, current_row, close));
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
                Interest::new(region),
                move || current.row.borrow().clone(),
                move || {
                    closing.deliveries.set(closing.deliveries.get() + 1);
                    *closing.row.borrow_mut() = None;
                },
            )
        }
    }

    /// Exercise exact producer-row selection through the real traversal.
    /// Returning None makes a missing fixture opening fail before delivery.
    fn capture_row(owner: &MovementOwner, row: &str) -> Option<MovementTicket> {
        let ticket =
            owner.capture_affected(&MovedRows::new(AnchorRegion::Sessions, [row.to_owned()]));
        (!ticket.0.is_empty()).then_some(ticket)
    }

    /// Optional scoped admission must not narrow any existing close policy,
    /// including closed-slot writes and a retained open slot without row DOM.
    #[farhelm_testtrace::test]
    fn legacy_dispatch_ignores_absent_or_ineligible_anchor_bindings() {
        let owner = MovementOwner::default();
        let slot = Slot::default();
        let registration = slot.register(&owner, AnchorRegion::Sessions);
        slot.open("a");
        owner
            .capture_region(AnchorRegion::Sessions)
            .close_if_current();
        assert_eq!(slot.deliveries.get(), 0);
        registration.close_if_open();
        assert_eq!(slot.deliveries.get(), 1);
        slot.open("a");
        let _anchor = registration.bind_anchor("a".into(), || false).unwrap();
        owner
            .capture_region(AnchorRegion::Sessions)
            .close_if_current();
        assert_eq!(slot.deliveries.get(), 1);
        owner.close_affected(&MovedRows::new(AnchorRegion::Sessions, ["a".into()]));
        assert_eq!(slot.deliveries.get(), 2);
        owner.close_registered(AnchorRegion::Sessions);
        assert_eq!(
            slot.deliveries.get(),
            3,
            "legacy unconditional writes remain unconditional"
        );
    }

    /// A new report addresses the captured opening, not the latest same-row
    /// slot. Pending and fallback are included: binding takes no placement
    /// policy, and neither outcome replaces the measurement-start lease.
    #[farhelm_testtrace::test]
    fn reopening_revokes_old_tickets_before_the_next_measurement() {
        let owner = MovementOwner::default();
        let slot = Slot::default();
        let registration = slot.register(&owner, AnchorRegion::Sessions);
        slot.open("a");
        let old = registration.bind_anchor("a".into(), || true).unwrap();
        let pending_ticket = capture_row(&owner, "a").unwrap();
        let fallback_ticket = capture_row(&owner, "a").unwrap();
        registration.forget_anchor();
        slot.open("a");
        assert!(capture_row(&owner, "a").is_none());
        pending_ticket.close_if_current();
        fallback_ticket.close_if_current();
        assert_eq!(slot.deliveries.get(), 0);
        let _fresh = registration.bind_anchor("a".into(), || true).unwrap();
        drop(old);
        capture_row(&owner, "a").unwrap().close_if_current();
        assert_eq!(
            slot.deliveries.get(),
            1,
            "old cleanup cannot clear the fresh same-row lease"
        );
    }

    /// Every bell shares its parent's registration. A's delayed callback or
    /// width producer must neither replace nor capture the current bell B.
    #[farhelm_testtrace::test]
    fn late_bind_and_capture_are_constrained_to_the_actual_current_row() {
        let owner = MovementOwner::default();
        let slot = Slot::default();
        let registration = slot.register(&owner, AnchorRegion::Sessions);
        slot.open("a");
        let a = registration.bind_anchor("a".into(), || true).unwrap();
        let a_ticket = capture_row(&owner, "a").unwrap();
        registration.forget_anchor();
        slot.open("b");
        let _b = registration.bind_anchor("b".into(), || true).unwrap();
        assert!(registration.bind_anchor("a".into(), || true).is_none());
        assert!(capture_row(&owner, "a").is_none());
        a_ticket.close_if_current();
        drop(a);
        assert_eq!(slot.deliveries.get(), 0);
        capture_row(&owner, "b").unwrap().close_if_current();
        assert_eq!(slot.deliveries.get(), 1);
    }

    /// Regional and selected reports share the same opaque row traversal.
    /// A matching id in another region and a later replacement are excluded;
    /// future host enrollment therefore needs no session-specific owner rule.
    #[farhelm_testtrace::test]
    fn affected_capture_keeps_region_membership_and_delivery_row_identity() {
        let owner = MovementOwner::default();
        let first = Slot::default();
        let lower = Slot::default();
        let replaced = Slot::default();
        let session = Slot::default();
        first.open("a");
        lower.open("b");
        replaced.open("c");
        session.open("b");
        let a = first.register(&owner, AnchorRegion::Hosts);
        let b = lower.register(&owner, AnchorRegion::Hosts);
        let c = replaced.register(&owner, AnchorRegion::Hosts);
        let s = session.register(&owner, AnchorRegion::Sessions);
        let _a = a.bind_anchor("a".into(), || true).unwrap();
        let _b = b.bind_anchor("b".into(), || true).unwrap();
        let _c = c.bind_anchor("c".into(), || true).unwrap();
        let _s = s.bind_anchor("b".into(), || true).unwrap();
        let selected = owner.capture_affected(&MovedRows::new(
            AnchorRegion::Hosts,
            ["b".into(), "c".into()],
        ));
        assert_eq!(
            selected.0.len(),
            2,
            "exactly the affected host openings were captured"
        );
        c.forget_anchor();
        replaced.open("d");
        let _d = c.bind_anchor("d".into(), || true).unwrap();
        selected.close_if_current();
        assert_eq!(
            (
                first.deliveries.get(),
                lower.deliveries.get(),
                replaced.deliveries.get(),
                session.deliveries.get()
            ),
            (0, 1, 0, 0)
        );
        owner.capture_region(AnchorRegion::Hosts).close_if_current();
        assert_eq!(
            (
                first.deliveries.get(),
                replaced.deliveries.get(),
                session.deliveries.get()
            ),
            (1, 1, 0)
        );
    }

    /// Regional content reports select admitted Sessions only. Weak tickets
    /// cannot retain row-local readers after teardown, even with open ids left.
    #[farhelm_testtrace::test]
    fn scope_drop_releases_reader_and_regional_capture_excludes_other_regions() {
        let owner = MovementOwner::default();
        let slot = Slot::default();
        let registration = slot.register(&owner, AnchorRegion::Sessions);
        slot.open("a");
        let witness = Rc::new(());
        let weak = Rc::downgrade(&witness);
        let row = registration
            .bind_anchor("a".into(), move || {
                let _ = &witness;
                true
            })
            .unwrap();
        assert!(
            weak.upgrade().is_some(),
            "the live row owns the reader witness"
        );
        let ticket = owner.capture_region(AnchorRegion::Sessions);
        drop(row);
        assert!(
            weak.upgrade().is_none(),
            "tickets and entries keep only weak row bindings"
        );
        ticket.close_if_current();
        assert_eq!(slot.deliveries.get(), 0);
        let host = Slot::default();
        host.open("host");
        let host_registration = host.register(&owner, AnchorRegion::Hosts);
        let _host_anchor = host_registration
            .bind_anchor("host".into(), || true)
            .unwrap();
        let _row = registration.bind_anchor("a".into(), || true).unwrap();
        owner
            .capture_region(AnchorRegion::Sessions)
            .close_if_current();
        assert_eq!(slot.deliveries.get(), 1);
        assert_eq!(host.deliveries.get(), 0);
    }

    /// Reader/closer callbacks can revoke or unregister bindings. No RefCell
    /// borrow may cross those callbacks, and a captured retired entry is inert.
    #[farhelm_testtrace::test]
    fn admitted_dispatch_releases_borrows_and_rechecks_unregistration() {
        let owner = MovementOwner::default();
        let first = Slot::default();
        first.open("a");
        let retiring = Rc::new(RefCell::new(None::<Registration>));
        let retire = retiring.clone();
        let read_first = first.clone();
        let close_first = first.clone();
        let callback_owner = Rc::downgrade(&owner.0);
        let registration = owner.register(
            Interest::new(AnchorRegion::Sessions),
            move || read_first.row.borrow().clone(),
            move || {
                assert!(callback_owner.upgrade().unwrap().try_borrow_mut().is_ok());
                if let Some(registration) = retire.borrow_mut().take() {
                    registration.unregister();
                }
                close_first.deliveries.set(close_first.deliveries.get() + 1);
                *close_first.row.borrow_mut() = None;
            },
        );
        let callback_registration = registration.clone();
        let _first_anchor = registration
            .bind_anchor("a".into(), move || {
                let entry = callback_registration.entry().unwrap();
                assert!(entry.anchor.try_borrow_mut().is_ok());
                true
            })
            .unwrap();
        let second = Slot::default();
        second.open("b");
        let second_registration = second.register(&owner, AnchorRegion::Sessions);
        let _second_anchor = second_registration
            .bind_anchor("b".into(), || true)
            .unwrap();
        *retiring.borrow_mut() = Some(second_registration);
        let ticket = owner.capture_region(AnchorRegion::Sessions);
        ticket.close_if_current();
        assert_eq!(first.deliveries.get(), 1);
        assert_eq!(second.deliveries.get(), 0);
    }

    /// A real Dioxus row scope owns the production binding hook. Err disables
    /// admission before physical teardown; Ok alone cannot revive a dropped
    /// row, and a recovery measurement creates a distinct opening identity.
    #[farhelm_testtrace::test]
    fn component_binding_tracks_err_teardown_and_recovery_mount() {
        /// Control logical readiness separately from physical row presence.
        type BindingFixture = (Registration, Rc<Cell<bool>>, Rc<Cell<bool>>);
        thread_local! {
            static FIXTURE: RefCell<Option<BindingFixture>> = const { RefCell::new(None) };
        }
        #[component]
        fn BoundRow() -> Element {
            let (registration, ready, _) =
                FIXTURE.with(|fixture| fixture.borrow().clone().unwrap());
            let ready = use_callback(move |()| ready.get());
            let bind = use_anchor_binding(registration, "a".into(), ready);
            // This named boundary stands in for starting the existing renderer
            // measurement; the headless renderer supplies no MountedData rect.
            use_hook(move || bind.call(()));
            rsx! {}
        }
        fn app() -> Element {
            let shown = FIXTURE.with(|fixture| fixture.borrow().as_ref().unwrap().2.get());
            if shown {
                rsx! { BoundRow {} }
            } else {
                rsx! {}
            }
        }
        let owner = MovementOwner::default();
        let slot = Slot::default();
        slot.open("a");
        let registration = slot.register(&owner, AnchorRegion::Sessions);
        let ready = Rc::new(Cell::new(true));
        let shown = Rc::new(Cell::new(true));
        FIXTURE.with(|fixture| {
            *fixture.borrow_mut() = Some((registration.clone(), ready.clone(), shown.clone()))
        });
        let mut dom = VirtualDom::new(app);
        assert!(
            capture_row(&owner, "a").is_none(),
            "Ok before first mount is no anchor"
        );
        dom.rebuild_in_place();
        let old = capture_row(&owner, "a").unwrap();
        let before_err = capture_row(&owner, "a").unwrap();
        ready.set(false);
        before_err.close_if_current();
        owner
            .capture_region(AnchorRegion::Sessions)
            .close_if_current();
        assert_eq!(
            slot.deliveries.get(),
            0,
            "Err is excluded before the row drops"
        );
        shown.set(false);
        dom.mark_dirty(dioxus::core::ScopeId::APP);
        dom.render_immediate(&mut dioxus::core::NoOpMutations);
        ready.set(true);
        assert!(
            capture_row(&owner, "a").is_none(),
            "Ok before recovery mount is no anchor"
        );
        shown.set(true);
        dom.mark_dirty(dioxus::core::ScopeId::APP);
        dom.render_immediate(&mut dioxus::core::NoOpMutations);
        old.close_if_current();
        assert_eq!(
            slot.deliveries.get(),
            0,
            "a remount cannot reuse an old identity"
        );
        capture_row(&owner, "a").unwrap().close_if_current();
        assert_eq!(slot.deliveries.get(), 1);
        drop(dom);
        FIXTURE.with(|fixture| fixture.borrow_mut().take());
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
            Interest::new(AnchorRegion::Sessions),
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

    /// Direct disclosure is an earlier, unconditional action-panel policy:
    /// host closes before session even though broad layout uses registration
    /// order, and a bell survives until a later actual layout invalidation.
    #[farhelm_testtrace::test]
    fn direct_disclosure_preserves_audience_closed_writes_and_host_first_order() {
        let owner = MovementOwner::default();
        let events = Rc::new(RefCell::new(Vec::new()));
        let menu = Slot::default();
        let host = Slot::default();
        let bell = Slot::default();
        bell.open("bell-row");
        let _registrations: Vec<_> = [
            (AnchorRegion::Sessions, "session", true, menu.clone()),
            (AnchorRegion::Hosts, "host", true, host.clone()),
            (AnchorRegion::Sessions, "bell", false, bell.clone()),
        ]
        .into_iter()
        .map(|(region, label, direct_host_disclosure, slot)| {
            let current = slot.clone();
            let events = events.clone();
            owner.register(
                Interest {
                    region,
                    direct_host_disclosure,
                },
                move || current.row.borrow().clone(),
                move || {
                    events.borrow_mut().push(label);
                    slot.deliveries.set(slot.deliveries.get() + 1);
                    *slot.row.borrow_mut() = None;
                },
            )
        })
        .collect();
        assert!(menu.row.borrow().is_none() && host.row.borrow().is_none());
        owner.close_for_host_disclosure();
        assert_eq!(*events.borrow(), ["host", "session"]);
        assert_eq!(
            (
                menu.deliveries.get(),
                host.deliveries.get(),
                bell.deliveries.get()
            ),
            (1, 1, 0)
        );
        assert_eq!(bell.row.borrow().as_deref(), Some("bell-row"));

        events.borrow_mut().clear();
        menu.open("session-row");
        host.open("host-row");
        owner.close_open_in(&[AnchorRegion::Hosts, AnchorRegion::Sessions]);
        assert_eq!(*events.borrow(), ["session", "host", "bell"]);
    }

    /// A local epoch continuation selects its own registration, not every
    /// popup in the same region; repeated or retired closes deliver nothing.
    #[farhelm_testtrace::test]
    fn registration_epoch_continuations_remain_independent_and_live() {
        let owner = MovementOwner::default();
        let help = Slot::default();
        let update = Slot::default();
        let template = Slot::default();
        let help_registration = help.register(&owner, AnchorRegion::AppBar);
        let _update_registration = update.register(&owner, AnchorRegion::AppBar);
        let template_registration = template.register(&owner, AnchorRegion::Templates);
        help.open("help");
        update.open("update");
        template.open("template");
        help_registration.close_if_open();
        assert_eq!(help.deliveries.get(), 1);
        assert_eq!(update.row.borrow().as_deref(), Some("update"));
        assert_eq!(template.row.borrow().as_deref(), Some("template"));
        help_registration.close_if_open();
        assert_eq!(help.deliveries.get(), 1);
        help.open("help");
        help_registration.unregister();
        help_registration.close_if_open();
        assert_eq!(help.row.borrow().as_deref(), Some("help"));
        template_registration.close_if_open();
        assert_eq!(template.deliveries.get(), 1);
        assert_eq!(update.deliveries.get(), 0);
    }
}
