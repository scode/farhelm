//! The two panes' shared concurrency disciplines: the single live-operation
//! token ([`OpLock`], shared across the shell via [`PaneGate`]) and the
//! read generation gate ([`ReadGate`]).
//!
//! They solve mirrored halves of one problem. The token decides which
//! WRITES may run; the gate decides which READS may be believed. Both exist
//! because this page has several asynchronous things in flight against one
//! screen, and both are claimed synchronously — a value computed during a
//! render is already stale by the time a handler or a completion consults
//! it.
//!
//! ## What this replaces, and why a boolean prop could not do it
//!
//! The list page runs several mutually exclusive operations — a create, any
//! host mutation, an add-host — and the exclusion between them was a set of
//! booleans computed during RENDER and handed down as props (`blocked`,
//! `submitting`, `nav_locked`). That is a race wearing a guard's clothes.
//! A prop is a value captured when the component last rendered, so two
//! handlers firing before the next render both read `false` and both
//! proceed: a click on "add host" landing in the same frame as a create's
//! submit sees a page that was idle when it was drawn. Widening the render
//! gap is not even necessary to hit it — a queued click and a synthetic
//! double-click are both inside one frame.
//!
//! The token closes that by construction: the claim is a synchronous
//! test-and-set performed INSIDE the handler, at entry, against state that
//! no render sits between. Whoever claims it first wins; everyone else
//! returns immediately. The `disabled` attributes stay, but they are now
//! cosmetic — a reflection of the token for the user's benefit, never the
//! thing that enforces anything.
//!
//! ## What it covers, and what it deliberately does not
//!
//! It covers the operations that can invalidate each OTHER's premises: a
//! create choosing a target host, the five host mutations, and the add-host
//! form. A removal landing between a create's target being chosen and that
//! create reaching the helm files a session against a host that no longer
//! exists; an adopt landing there purges the cache the create is about to be
//! recorded in.
//!
//! It also gates the session-OPEN click, which starts nothing but swaps
//! everything: selecting a row replaces the keyed `SessionView` (tearing
//! down the previous one's tasks) and repaints the list an in-flight
//! mutation is about to reconcile. That one reads the token rather than
//! claiming it.
//!
//! Since the two-pane shell, the token is OWNED BY `AppBody` and shared
//! with the selected session's view (see [`PaneGate`]): the view's
//! rename/restart/archive claim the same token the list's create and host
//! mutations do, so neither pane can start a write under the other's, and
//! the open click's read covers view-side work too.
//!
//! Per-session stop/rename/delete are deliberately NOT under it. They are
//! scoped to their own row, cannot invalidate anything another row is doing,
//! and the browser suite pins that two rows' lifecycle actions stay usable
//! at once. They keep their per-session in-flight set, and the open click
//! consults both.

use dioxus::prelude::*;

/// The page's one live-operation token.
///
/// `Copy` because it is a handle to a signal rather than the state itself,
/// which is what lets it be handed to child components as an ordinary prop
/// while every holder claims and releases the SAME token.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct OpLock {
    held: Signal<bool>,
}

/// One claimed page operation, released even if its task is cancelled.
///
/// Component-scoped futures are dropped when their component disappears. A
/// manual `release()` at the end of such a future is therefore not a release
/// guarantee: removing the row that owns it can skip that line and leave the
/// whole page inert. Keeping the claim in the future's owned state makes
/// cancellation take the same path as every ordinary return.
pub(crate) struct OpGuard {
    held: Signal<bool>,
}

impl Drop for OpGuard {
    fn drop(&mut self) {
        self.held.set(false);
    }
}

/// Create the token. A hook: call it unconditionally, once, in the component
/// that owns the page (`list::ListView`).
pub(crate) fn use_op_lock() -> OpLock {
    OpLock {
        held: use_signal(|| false),
    }
}

impl OpLock {
    /// Claim the token and return its cancellation-safe release guard.
    ///
    /// Move the guard into the spawned future. Dropping that future then
    /// releases the claim before a removed component can strand it.
    pub(crate) fn claim_guard(&mut self) -> Option<OpGuard> {
        self.claim().then_some(OpGuard { held: self.held })
    }

    /// Claim the token, or report that someone else holds it.
    ///
    /// The whole mechanism is in this one function being a test-and-SET
    /// under a single write borrow, called synchronously at handler entry:
    /// there is no await, no render, and no other handler able to interleave
    /// between the check and the claim. A caller that gets `false` must
    /// return without side effects — it has not started an operation and
    /// must not undo one.
    ///
    /// Callers pair this with [`Self::release`] on every path out of the
    /// operation, success and failure alike. A leaked token would leave the
    /// page permanently inert, which is why the release sits at the end of
    /// the spawned task rather than beside any one of its outcomes.
    pub(crate) fn claim(&mut self) -> bool {
        claim_in(&mut self.held.write())
    }

    /// Release the token. Idempotent, so a path that releases twice is
    /// harmless — the failure worth preventing is a path that releases
    /// never.
    pub(crate) fn release(&mut self) {
        self.held.set(false);
    }

    /// Whether an operation is live, for a HANDLER.
    ///
    /// `peek`, deliberately: a handler is not a reactive scope and must not
    /// subscribe the component that happens to be rendering. Used by the
    /// session-open click, which does not claim the token but must not run
    /// while one is held.
    pub(crate) fn busy_now(&self) -> bool {
        *self.held.peek()
    }

    /// Whether an operation is live, for a RENDER.
    ///
    /// A tracked read, so the controls that reflect it re-render when it
    /// changes. Everything this feeds is cosmetic — see the module docs.
    pub(crate) fn busy(&self) -> bool {
        *self.held.read()
    }
}

/// The session view's face of the SHARED cross-pane write gate.
///
/// Under the two-pane shell both `ListView` and the selected `SessionView`
/// can mutate the same fleet at once, from surfaces that used to be
/// mutually exclusive pages with a private token each. `AppBody` therefore
/// owns ONE `OpLock` and hands it to both panes; this wrapper is what the
/// session view holds, and it adds the one rule the bare token cannot
/// express: the sidebar's per-row stop/delete/rename/archive deliberately
/// do NOT claim the token (two rows' operations must stay concurrent — a
/// pinned property), so the view's claim must ALSO refuse while any of
/// those row operations is in flight. `row_ops` is the live count the list
/// maintains for exactly that question.
///
/// The asymmetry is deliberate: row operations check the token but never
/// hold it, so they stay concurrent with each other while still being
/// refused during a view-side operation — and the view is refused during
/// theirs. Same-frame races resolve through the token's synchronous
/// test-and-set plus a `peek` of the count, both consulted inside the
/// handler, never through render-time booleans.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct PaneGate {
    lock: OpLock,
    row_ops: Signal<u32>,
}

impl PaneGate {
    pub(crate) fn new(lock: OpLock, row_ops: Signal<u32>) -> Self {
        Self { lock, row_ops }
    }

    /// Claim the shared token, refusing while any sidebar row operation is
    /// in flight. Same contract as [`OpLock::claim`] otherwise.
    pub(crate) fn claim(&mut self) -> bool {
        if *self.row_ops.peek() > 0 {
            return false;
        }
        self.lock.claim()
    }

    /// Release the shared token — see [`OpLock::release`].
    pub(crate) fn release(&mut self) {
        self.lock.release();
    }

    /// Handler-time busyness: the token OR a row operation. `peek`s, like
    /// [`OpLock::busy_now`], and for the same reason.
    pub(crate) fn busy_now(&self) -> bool {
        self.lock.busy_now() || *self.row_ops.peek() > 0
    }

    /// Render-time busyness for the view's disabled attributes — cosmetic,
    /// like [`OpLock::busy`]; the claim is what enforces.
    pub(crate) fn busy(&self) -> bool {
        self.lock.busy() || *self.row_ops.read() > 0
    }
}

/// Which of several in-flight READS of one resource may be believed.
///
/// Two numbers, because two questions have different answers.
///
/// - A **success** is committed only if its generation beats the newest one
///   already applied. That is what stops an older poll's body from
///   resurrecting what a mutation-triggered refresh just removed — the
///   registry a stale read describes has since been changed by something
///   this client did.
/// - A **failure** is reported if its generation is at least the newest
///   applied. This is the split that a single "is this the newest request"
///   check gets wrong in both directions: gating failures on being the
///   newest STARTED read discards a real failure whenever another request
///   has already begun (so a helm that is down looks merely quiet), while
///   gating successes the same way discards a perfectly good newer snapshot
///   just because a later request has since started and may yet fail.
///
/// The result is the behavior the surfaces want: a later failed poll leaves
/// the newer successful snapshot on screen AND says the refresh failed —
/// which is exactly what `hosts::HostsRead` is built to represent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct ReadGate {
    /// Generations handed out so far. Monotonic; never reused.
    started: u64,
    /// The newest generation whose SUCCESS has been committed.
    applied: u64,
}

impl ReadGate {
    /// Claim the next generation. Called synchronously when a read is
    /// STARTED, so ordering follows the order requests were asked for
    /// rather than the order their tasks happen to be scheduled.
    pub(crate) fn start(&mut self) -> u64 {
        self.started += 1;
        self.started
    }

    /// Whether a success from `generation` should be committed — and, if so,
    /// record it as the newest applied.
    pub(crate) fn accept_success(&mut self, generation: u64) -> bool {
        if generation <= self.applied {
            return false;
        }
        self.applied = generation;
        true
    }

    /// Whether a failure from `generation` should be reported.
    ///
    /// Deliberately does not mutate: a failure supersedes nothing, so the
    /// next success from any later generation still applies.
    pub(crate) fn accept_failure(&self, generation: u64) -> bool {
        generation >= self.applied
    }
}

/// The test-and-set itself, over a plain `&mut bool`.
///
/// Split out from [`OpLock::claim`] so the RULE can be exercised without a
/// Dioxus runtime to own a signal in. That split is not only for testing:
/// it also makes the atomicity visible in one place — the check and the set
/// are one expression over one borrow, and the signal is nothing but where
/// that bool lives.
fn claim_in(held: &mut bool) -> bool {
    if *held {
        return false;
    }
    *held = true;
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A token can be held by exactly one claimant at a time, and a release
    /// hands it to the next.
    ///
    /// The middle assertion is the whole point of the type: the second
    /// claim, made with NO render in between — exactly the situation a
    /// render-time boolean cannot see, because the boolean it would have
    /// read was computed before the first claim existed — is refused.
    #[test]
    fn one_claimant_at_a_time_with_no_render_in_between() {
        let mut held = false;
        assert!(claim_in(&mut held), "an idle page grants the token");
        assert!(held);
        assert!(
            !claim_in(&mut held),
            "a second handler in the same frame must be refused"
        );

        // Release is a plain clear, so the next operation may proceed. It is
        // idempotent by construction — the failure worth preventing is a
        // path that never releases at all, not one that releases twice.
        held = false;
        assert!(claim_in(&mut held));
    }

    /// An older read's SUCCESS must never overwrite a newer one.
    ///
    /// This is the resurrection bug in one assertion: a poll that left
    /// before a removal, completing after the removal's own refresh, still
    /// describes the host that was removed. Committing it puts the row back
    /// on screen until the next tick, which reads as the helm ignoring the
    /// removal.
    #[test]
    fn an_older_success_never_overwrites_a_newer_one() {
        let mut gate = ReadGate::default();
        let first = gate.start();
        let second = gate.start();

        assert!(gate.accept_success(second), "the newer read applies");
        assert!(
            !gate.accept_success(first),
            "the older read, completing late, describes a state that has since changed"
        );
        assert!(
            !gate.accept_success(second),
            "and a duplicate completion of the applied generation changes nothing"
        );
    }

    /// A failure that POSTDATES the applied snapshot is reported while that
    /// snapshot stays — the split the surfaces are built around.
    ///
    /// Gating failures on "is this the newest started read" instead would
    /// discard this one the moment the next poll began, so a helm that is
    /// down would look merely quiet: rows on screen, no error, nothing to
    /// act on.
    #[test]
    fn a_later_failure_is_reported_without_disturbing_the_applied_snapshot() {
        let mut gate = ReadGate::default();
        let first = gate.start();
        assert!(gate.accept_success(first));

        let second = gate.start();
        // A third read has already started by the time the second fails,
        // which is the ordinary case at a three-second cadence.
        let _third = gate.start();
        assert!(
            gate.accept_failure(second),
            "a failure newer than what is displayed is worth saying"
        );
        assert!(
            gate.accept_success(gate.started),
            "and the snapshot the next success brings still applies"
        );
    }

    /// A failure from a read OLDER than the applied snapshot is dropped: it
    /// describes a moment that has already been superseded by a success, and
    /// reporting it would put a refresh-failed line over rows that were just
    /// refreshed successfully.
    #[test]
    fn a_failure_older_than_the_applied_snapshot_is_dropped() {
        let mut gate = ReadGate::default();
        let stale = gate.start();
        let fresh = gate.start();
        assert!(gate.accept_success(fresh));
        assert!(!gate.accept_failure(stale));
    }
}
