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
//! rename/restart claim the same token the list's create and host
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
        claimed_owner(self.claim(), || OpGuard { held: self.held })
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
/// express: the sidebar's per-row stop/delete/rename deliberately
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
    /// in flight, and return the cancellation-safe release guard
    /// ([`OpGuard`]). Otherwise the same contract as [`OpLock::claim_guard`].
    ///
    /// The only way the session view claims the token: the guard is held by
    /// a [`ConfirmSlot`] while a prompt is open, by the view's own claim slot
    /// while a Restart is pending, or by the task a confirmation starts. Each
    /// of those is dropped with the view, so no claim can outlive it. There
    /// is deliberately no bare `claim`/`release` pair here: a hand-released
    /// claim is exactly what used to strand the token when the view unmounted
    /// mid-operation and left every write action in the window disabled.
    pub(crate) fn claim_guard(&mut self) -> Option<OpGuard> {
        if *self.row_ops.peek() > 0 {
            return None;
        }
        self.lock.claim_guard()
    }

    /// [`Self::claim_guard`] into a slot the claiming component owns,
    /// returning whether the claim succeeded. Releasing is setting the slot
    /// to `None`; the slot is a signal of that component, so when the
    /// component unmounts the guard is dropped with it and the claim cannot
    /// outlive it. This is how the session view holds its Restart claims.
    pub(crate) fn claim_into(&mut self, slot: &mut Signal<Option<OpGuard>>) -> bool {
        match self.claim_guard() {
            Some(guard) => {
                slot.set(Some(guard));
                true
            }
            None => false,
        }
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

    /// Supersede every read that started before a locally confirmed mutation.
    ///
    /// A mutation is newer evidence than any request already in flight, even
    /// when its response cannot be decoded into a replacement snapshot. The
    /// next read receives a later generation and may apply normally; older
    /// successes and failures are both rejected when they eventually land.
    pub(crate) fn fence(&mut self) {
        self.started += 1;
        self.applied = self.started;
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

/// One inline "confirm / cancel" prompt: which target it is open for, and
/// whatever the confirmation will own when it proceeds.
///
/// Every confirmation in this UI is two buttons whose clicks can arrive in
/// the same event burst, before the render that removes them. A cancel
/// queued just ahead of a confirm must win, and so must a confirm queued
/// ahead of a cancel: the first click decides, the second must find nothing
/// to act on. Hand-rolling that check in each handler is how the header's
/// Replace shipped without it (cancel, then a queued confirm, still replaced
/// the session) and how the lifecycle prompts' cancel handlers kept
/// releasing the page's operation lock after a confirm had already handed
/// it to a running task. This type makes the check the only way in, the way
/// [`OpLock`] did for operation exclusion:
///
/// - [`Self::take`] is the one way a confirm handler learns the user
///   confirmed THIS prompt. It is a synchronous test-and-clear: it yields
///   the payload only if the prompt is still open for `key`, and leaves any
///   other prompt untouched.
/// - [`Self::cancel_for`] closes the prompt only if it is still open for
///   `key`; after a `take` it does nothing.
/// - The payload is what the confirmation owns. For the lifecycle prompts it
///   is the [`OpGuard`] claimed when the prompt opened: it lives in the slot
///   while the prompt is open, moves into the confirmed task on `take`, and
///   is dropped (releasing the lock) by cancel, by [`Self::clear`], by
///   replacement, or by the slot itself going away with its component. No
///   handler releases the lock by hand, so none can release someone else's.
///
/// `Copy` because it is a handle to a signal, like [`OpLock`]. Render code
/// uses the tracked readers ([`Self::is_open`], [`Self::current_key`]);
/// handlers use [`Self::take`] and [`Self::cancel_for`], which `peek`.
pub(crate) struct ConfirmSlot<K: 'static, P: 'static = ()> {
    open: Signal<Option<(K, P)>>,
}

impl<K: 'static, P: 'static> Clone for ConfirmSlot<K, P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K: 'static, P: 'static> Copy for ConfirmSlot<K, P> {}

impl<K: 'static, P: 'static> PartialEq for ConfirmSlot<K, P> {
    fn eq(&self, other: &Self) -> bool {
        self.open == other.open
    }
}

/// Close `slot`, dropping whatever it holds, whenever `shown` reads false.
/// A hook: call it unconditionally, once, in the component that owns the
/// slot.
///
/// For a prompt drawn on a surface that can stop rendering while its
/// component stays mounted (the session view's interrupted-session card,
/// which disappears when the session is restarted elsewhere): nothing on the
/// vanished surface can close the prompt any more, so without this its
/// claim would stay held. `shown` is read reactively, so the effect reruns
/// when what it reads changes.
pub(crate) fn use_clear_when_hidden<K: PartialEq + Clone + 'static, P: 'static>(
    mut slot: ConfirmSlot<K, P>,
    shown: impl Fn() -> bool + 'static,
) {
    use_effect(move || {
        if !shown() {
            slot.clear();
        }
    });
}

/// Create a closed prompt slot. A hook: call it unconditionally, once, in
/// the component whose prompt it is. The slot's contents (a held
/// [`OpGuard`], say) are dropped when that component unmounts.
pub(crate) fn use_confirm_slot<K: 'static, P: 'static>() -> ConfirmSlot<K, P> {
    ConfirmSlot {
        open: use_signal(|| None),
    }
}

impl<K: PartialEq + Clone + 'static, P: 'static> ConfirmSlot<K, P> {
    /// Open the prompt for `key`, holding `payload` until it is confirmed or
    /// dismissed. An already open prompt is REPLACED (the tab strip moves
    /// its close prompt to the tab clicked last) and its payload dropped.
    /// For a slot whose payload is an operation claim that drop cannot
    /// release anyone else's claim: a second prompt could only have claimed
    /// after the first one's claim was gone.
    pub(crate) fn open(&mut self, key: K, payload: P) {
        open_in(&mut self.open.write(), key, payload);
    }

    /// Consume the confirmation for `key`: the payload if the prompt is
    /// still open for exactly that key, `None` (and nothing changed)
    /// otherwise.
    pub(crate) fn take(&mut self, key: &K) -> Option<P> {
        if !self
            .open
            .peek()
            .as_ref()
            .is_some_and(|(open, _)| open == key)
        {
            return None;
        }
        take_in(&mut self.open.write(), key)
    }

    /// Dismiss the prompt if it is still open for `key`, dropping its
    /// payload. Does nothing once a confirmation has taken it.
    pub(crate) fn cancel_for(&mut self, key: &K) {
        drop(self.take(key));
    }

    /// Close whatever is open, unconditionally. For reconciliation (the
    /// target disappeared), never for an event handler: a handler must name
    /// the prompt it was rendered for, which is what `cancel_for` does.
    pub(crate) fn clear(&mut self) {
        if self.open.peek().is_some() {
            self.open.set(None);
        }
    }

    /// Whether any prompt is open, for a RENDER (tracked read).
    pub(crate) fn is_open(&self) -> bool {
        self.open.read().is_some()
    }

    /// Which key the prompt is open for, for a RENDER (tracked read).
    pub(crate) fn current_key(&self) -> Option<K> {
        self.open.read().as_ref().map(|(key, _)| key.clone())
    }
}

/// [`ConfirmSlot::open`]'s state change over a plain `Option`, split out
/// (like [`claim_in`]) so the rules are testable without a Dioxus runtime.
fn open_in<K, P>(slot: &mut Option<(K, P)>, key: K, payload: P) {
    *slot = Some((key, payload));
}

/// [`ConfirmSlot::take`]'s test-and-clear over a plain `Option`.
fn take_in<K: PartialEq, P>(slot: &mut Option<(K, P)>, key: &K) -> Option<P> {
    if slot.as_ref().is_some_and(|(open, _)| open == key) {
        slot.take().map(|(_, payload)| payload)
    } else {
        None
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

/// Construct release ownership only after the corresponding claim succeeds.
///
/// The laziness is the contract. An eager `then_some(OpGuard { .. })` creates
/// and immediately drops a guard when a second claim is refused; that drop
/// releases the first caller's live operation. Keeping construction behind a
/// closure makes the failed-claim path incapable of manufacturing release
/// authority it never acquired.
fn claimed_owner<T>(claimed: bool, owner: impl FnOnce() -> T) -> Option<T> {
    claimed.then(owner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dioxus::prelude::VirtualDom;

    /// A payload that records its own drop, standing in for an `OpGuard`
    /// so the slot rules can be checked without a Dioxus runtime.
    struct Owned(std::rc::Rc<std::cell::Cell<bool>>);

    impl Drop for Owned {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }

    fn owned() -> (Owned, std::rc::Rc<std::cell::Cell<bool>>) {
        let dropped = std::rc::Rc::new(std::cell::Cell::new(false));
        (Owned(dropped.clone()), dropped)
    }

    /// Why this matters: a cancel click and a confirm click can land in one
    /// event burst. Spec (`ConfirmSlot`): whichever runs first decides. A
    /// confirm after a cancel finds nothing and proceeds with nothing; a
    /// cancel after a confirm finds nothing and releases nothing, so the
    /// confirmed task keeps its claim.
    #[farhelm_testtrace::test]
    fn the_first_of_a_confirm_and_a_cancel_decides() {
        // cancel, then confirm
        let (payload, dropped) = owned();
        let mut slot = None;
        open_in(&mut slot, "prompt", payload);
        drop(take_in(&mut slot, &"prompt")); // cancel_for
        assert!(dropped.get(), "cancel releases what the prompt held");
        assert!(
            take_in(&mut slot, &"prompt").is_none(),
            "a queued confirm finds nothing"
        );

        // confirm, then cancel
        let (payload, dropped) = owned();
        open_in(&mut slot, "prompt", payload);
        let confirmed = take_in(&mut slot, &"prompt").expect("the confirm takes the payload");
        drop(take_in(&mut slot, &"prompt")); // the queued cancel
        assert!(
            !dropped.get(),
            "a queued cancel must not release the confirmed task's claim"
        );
        drop(confirmed);
        assert!(dropped.get());
    }

    /// Why this matters: the second click of a double-click, or a stale
    /// handler rendered for another target, must not act on the prompt that
    /// is open now. Spec: a duplicate confirm gets nothing; a confirm or
    /// cancel naming a different key leaves the open prompt and its payload
    /// untouched.
    #[farhelm_testtrace::test]
    fn duplicate_and_wrong_key_clicks_change_nothing() {
        let (payload, dropped) = owned();
        let mut slot = None;
        open_in(&mut slot, "tab-a", payload);
        assert!(
            take_in(&mut slot, &"tab-b").is_none(),
            "a wrong key takes nothing"
        );
        drop(take_in(&mut slot, &"tab-b")); // a cancel rendered for tab-b
        assert!(
            !dropped.get() && slot.is_some(),
            "the open prompt survives other keys"
        );
        let first = take_in(&mut slot, &"tab-a");
        assert!(first.is_some());
        assert!(
            take_in(&mut slot, &"tab-a").is_none(),
            "a duplicate confirm gets nothing"
        );
    }

    /// Why this matters: a lifecycle prompt holds the page's operation claim
    /// while it is open, and the session view that owns it can unmount
    /// mid-prompt (the user opens another session). A claim stranded by that
    /// would leave the whole page inert. Spec: when the component owning an
    /// open `ConfirmSlot` goes away, the `OpGuard` it held is dropped with it
    /// and the lock is free again; while it is mounted, the lock stays held.
    #[farhelm_testtrace::test]
    fn an_unmounted_prompt_releases_the_claim_it_held() {
        use std::cell::Cell;
        std::thread_local! {
            static SHOW: Cell<bool> = const { Cell::new(true) };
            static HELD: Cell<Option<bool>> = const { Cell::new(None) };
        }

        #[component]
        fn Prompt(lock: OpLock) -> Element {
            let mut slot: ConfirmSlot<(), OpGuard> = use_confirm_slot();
            use_hook(move || {
                let mut lock = lock;
                if let Some(claim) = lock.claim_guard() {
                    slot.open((), claim);
                }
            });
            rsx! {}
        }

        fn app() -> Element {
            let lock = use_op_lock();
            HELD.with(|held| held.set(Some(lock.busy_now())));
            let show = SHOW.with(Cell::get);
            rsx! {
                if show {
                    Prompt { lock }
                }
            }
        }

        let mut dom = VirtualDom::new(app);
        dom.rebuild_to_vec();
        // Re-render the parent so it observes the claim the child took.
        dom.mark_dirty(dioxus::core::ScopeId::APP);
        dom.render_immediate(&mut dioxus::core::NoOpMutations);
        assert_eq!(
            HELD.with(Cell::get),
            Some(true),
            "premise: the open prompt holds the claim"
        );

        SHOW.with(|show| show.set(false));
        dom.mark_dirty(dioxus::core::ScopeId::APP);
        dom.render_immediate(&mut dioxus::core::NoOpMutations);
        dom.mark_dirty(dioxus::core::ScopeId::APP);
        dom.render_immediate(&mut dioxus::core::NoOpMutations);
        assert_eq!(
            HELD.with(Cell::get),
            Some(false),
            "unmounting the prompt must release the claim it held"
        );
    }

    /// A claim on one shared lock cell, released on drop: the pure-rule
    /// stand-in for an `OpGuard` over an `OpLock`.
    struct Claim(std::rc::Rc<std::cell::Cell<bool>>);

    impl Drop for Claim {
        fn drop(&mut self) {
            self.0.set(false);
        }
    }

    /// Claim the shared cell with the same test-and-set `OpLock` uses.
    fn claim(lock: &std::rc::Rc<std::cell::Cell<bool>>) -> Option<Claim> {
        let mut held = lock.get();
        let claimed = claim_in(&mut held);
        lock.set(held);
        claimed_owner(claimed, || Claim(lock.clone()))
    }

    /// Why this matters: a confirmed lifecycle prompt hands its claim to the
    /// task it starts (the Replace request), and the manual release that
    /// task used to end with is gone, so the claim must live exactly as long
    /// as the task: held while the request is pending, released when the
    /// owning view unmounts and cancels it. Spec: after `take`, the slot is
    /// empty and the lock stays held across a pending task; unmounting the
    /// component cancels the task and releases the lock.
    #[farhelm_testtrace::test]
    async fn a_confirmed_task_holds_the_claim_until_it_is_cancelled() {
        use std::cell::Cell;
        std::thread_local! {
            static SHOW: Cell<bool> = const { Cell::new(true) };
            static HELD: Cell<Option<bool>> = const { Cell::new(None) };
            static SLOT_OPEN: Cell<Option<bool>> = const { Cell::new(None) };
            static REACHED: Cell<bool> = const { Cell::new(false) };
        }

        #[component]
        fn Confirmed(lock: OpLock) -> Element {
            let mut slot: ConfirmSlot<(), OpGuard> = use_confirm_slot();
            let mut reached = use_signal(|| false);
            use_hook(move || {
                let mut lock = lock;
                if let Some(claim) = lock.claim_guard() {
                    slot.open((), claim);
                }
                // The confirm click: the claim moves into a task that stays
                // pending, like a Replace request still in flight.
                if let Some(claim) = slot.take(&()) {
                    spawn(async move {
                        let _claim = claim;
                        reached.set(true);
                        std::future::pending::<()>().await;
                    });
                }
            });
            SLOT_OPEN.with(|open| open.set(Some(slot.is_open())));
            REACHED.with(|seen| seen.set(*reached.read()));
            rsx! {}
        }

        fn app() -> Element {
            let lock = use_op_lock();
            HELD.with(|held| held.set(Some(lock.busy_now())));
            let show = SHOW.with(Cell::get);
            rsx! {
                if show {
                    Confirmed { lock }
                }
            }
        }

        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        // Drive the runtime until the task has run up to its pending point;
        // its signal write is what wakes `wait_for_work`, so no sleep.
        while !REACHED.with(Cell::get) {
            tokio::time::timeout(std::time::Duration::from_secs(5), dom.wait_for_work())
                .await
                .expect("the confirmed task reaches its pending point");
            dom.render_immediate(&mut dioxus::core::NoOpMutations);
        }
        dom.mark_dirty(dioxus::core::ScopeId::APP);
        dom.render_immediate(&mut dioxus::core::NoOpMutations);
        assert_eq!(
            SLOT_OPEN.with(Cell::get),
            Some(false),
            "the confirm emptied the slot"
        );
        assert_eq!(
            HELD.with(Cell::get),
            Some(true),
            "the pending task still holds the claim"
        );

        SHOW.with(|show| show.set(false));
        dom.mark_dirty(dioxus::core::ScopeId::APP);
        dom.render_immediate(&mut dioxus::core::NoOpMutations);
        dom.mark_dirty(dioxus::core::ScopeId::APP);
        dom.render_immediate(&mut dioxus::core::NoOpMutations);
        assert_eq!(
            HELD.with(Cell::get),
            Some(false),
            "unmounting cancels the task, and the claim goes with it"
        );
    }

    /// Why this matters: a prompt that holds the page's operation claim must
    /// never be able to release a LATER owner's claim. Spec: while a prompt
    /// is open nobody else can claim; once its confirmed task finishes and
    /// someone else takes the lock, a stale cancel aimed at the old prompt
    /// finds nothing and the new owner keeps the lock (another claim is
    /// still refused).
    #[farhelm_testtrace::test]
    fn a_refused_claim_opens_nothing_and_a_later_owner_is_protected() {
        let lock = std::rc::Rc::new(std::cell::Cell::new(false));
        let mut slot = None;
        open_in(
            &mut slot,
            (),
            claim(&lock).expect("the first prompt claims"),
        );
        assert!(
            claim(&lock).is_none(),
            "a refused claim yields nothing to open a prompt with"
        );

        let task = take_in(&mut slot, &()).expect("confirmed");
        drop(task); // the confirmed task finishes and releases
        assert!(!lock.get());

        let later_owner = claim(&lock).expect("a later operation claims the lock");
        drop(take_in(&mut slot, &())); // a stale cancel for the old prompt
        assert!(
            lock.get(),
            "the stale cancel must not release the later owner's claim"
        );
        assert!(
            claim(&lock).is_none(),
            "the lock is still held against a third claim"
        );
        drop(later_owner);
        assert!(!lock.get());
    }

    /// A token can be held by exactly one claimant at a time, and a release
    /// hands it to the next.
    ///
    /// The middle assertion is the whole point of the type: the second
    /// claim, made with NO render in between — exactly the situation a
    /// render-time boolean cannot see, because the boolean it would have
    /// read was computed before the first claim existed — is refused.
    #[farhelm_testtrace::test]
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

    /// A refused claim must not construct anything whose destructor could
    /// release the operation that already owns the token.
    ///
    /// This pins the eager-`then_some` failure mode: evaluating the owner on
    /// `false` briefly creates release authority and drops it immediately,
    /// clearing the first operation's lock before that operation completes.
    #[farhelm_testtrace::test]
    fn a_refused_claim_never_constructs_release_ownership() {
        let constructed = std::cell::Cell::new(false);

        let owner = claimed_owner(false, || {
            constructed.set(true);
        });

        assert!(owner.is_none());
        assert!(
            !constructed.get(),
            "a failed claimant has no release authority to construct or drop"
        );
    }

    /// An older read's SUCCESS must never overwrite a newer one.
    ///
    /// This is the resurrection bug in one assertion: a poll that left
    /// before a removal, completing after the removal's own refresh, still
    /// describes the host that was removed. Committing it puts the row back
    /// on screen until the next tick, which reads as the helm ignoring the
    /// removal.
    #[farhelm_testtrace::test]
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
    #[farhelm_testtrace::test]
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
    #[farhelm_testtrace::test]
    fn a_failure_older_than_the_applied_snapshot_is_dropped() {
        let mut gate = ReadGate::default();
        let stale = gate.start();
        let fresh = gate.start();
        assert!(gate.accept_success(fresh));
        assert!(!gate.accept_failure(stale));
    }

    /// A mutation accepted after a GET starts is newer evidence than that
    /// GET's eventual body or failure. This pins the catalog race where an
    /// old response could otherwise undo the locally absorbed mutation until
    /// its follow-up refresh completed.
    #[farhelm_testtrace::test]
    fn a_mutation_fence_rejects_reads_that_started_before_it() {
        let mut gate = ReadGate::default();
        let old_get = gate.start();

        gate.fence();

        assert!(!gate.accept_success(old_get));
        assert!(!gate.accept_failure(old_get));
        let refresh = gate.start();
        assert!(gate.accept_success(refresh));
    }

    /// Why this matters: the session view's Restart controls hold the
    /// shared lock between a click and the end of the restart (the stop-first
    /// prompt, the "Restart with" dialog, the request in flight), and the
    /// view can unmount meanwhile (the session deleted from another client).
    /// The lock is owned by the page shell, so a hand-released claim used to
    /// outlive the view and leave every write action in the window disabled.
    /// Spec: a claim taken with `PaneGate::claim_into` into a slot the
    /// claiming component owns holds the lock while that component is
    /// mounted and is released when it unmounts.
    #[farhelm_testtrace::test]
    fn a_view_claim_is_released_when_the_view_unmounts() {
        use std::cell::Cell;
        std::thread_local! {
            static SHOW: Cell<bool> = const { Cell::new(true) };
            static HELD: Cell<Option<bool>> = const { Cell::new(None) };
        }

        #[component]
        fn View(gate: PaneGate) -> Element {
            let mut slot = use_signal(|| None::<OpGuard>);
            use_hook(move || {
                let mut gate = gate;
                assert!(gate.claim_into(&mut slot), "premise: the claim succeeds");
            });
            rsx! {}
        }

        fn app() -> Element {
            let lock = use_op_lock();
            let row_ops = use_signal(|| 0u32);
            HELD.with(|held| held.set(Some(lock.busy_now())));
            let show = SHOW.with(Cell::get);
            rsx! {
                if show {
                    View { gate: PaneGate::new(lock, row_ops) }
                }
            }
        }

        let mut dom = VirtualDom::new(app);
        dom.rebuild_to_vec();
        dom.mark_dirty(dioxus::core::ScopeId::APP);
        dom.render_immediate(&mut dioxus::core::NoOpMutations);
        assert_eq!(
            HELD.with(Cell::get),
            Some(true),
            "premise: the view holds the claim"
        );

        SHOW.with(|show| show.set(false));
        dom.mark_dirty(dioxus::core::ScopeId::APP);
        dom.render_immediate(&mut dioxus::core::NoOpMutations);
        dom.mark_dirty(dioxus::core::ScopeId::APP);
        dom.render_immediate(&mut dioxus::core::NoOpMutations);
        assert_eq!(
            HELD.with(Cell::get),
            Some(false),
            "unmounting the view must release the claim it held"
        );
    }

    /// Why this matters: the interrupted-session card's Replace prompt holds
    /// the shared lock, and the card stops rendering when the session is
    /// restarted elsewhere while the session view stays mounted. Nothing on
    /// the vanished card can close the prompt, so its claim used to stay
    /// held with the window's write actions disabled. Spec: a slot passed to
    /// `use_clear_when_hidden` keeps its claim while `shown` is true and is
    /// closed, releasing the claim, once `shown` turns false, with the owning
    /// component still mounted.
    #[farhelm_testtrace::test]
    async fn a_prompt_on_a_hidden_surface_is_closed_and_releases_its_claim() {
        use std::cell::Cell;
        std::thread_local! {
            static HELD: Cell<Option<bool>> = const { Cell::new(None) };
            static OPEN: Cell<Option<bool>> = const { Cell::new(None) };
            static SHOWN: Cell<Option<Signal<bool>>> = const { Cell::new(None) };
        }

        #[component]
        fn Card(lock: OpLock) -> Element {
            let mut slot: ConfirmSlot<(), OpGuard> = use_confirm_slot();
            let shown = use_signal(|| true);
            SHOWN.with(|cell| cell.set(Some(shown)));
            use_hook(move || {
                let mut lock = lock;
                if let Some(claim) = lock.claim_guard() {
                    slot.open((), claim);
                }
            });
            use_clear_when_hidden(slot, move || *shown.read());
            OPEN.with(|open| open.set(Some(slot.is_open())));
            rsx! {}
        }

        fn app() -> Element {
            let lock = use_op_lock();
            HELD.with(|held| held.set(Some(lock.busy_now())));
            rsx! { Card { lock } }
        }

        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        dom.mark_dirty(dioxus::core::ScopeId::APP);
        dom.render_immediate(&mut dioxus::core::NoOpMutations);
        assert_eq!(
            HELD.with(Cell::get),
            Some(true),
            "premise: the open prompt holds the claim"
        );
        assert_eq!(
            OPEN.with(Cell::get),
            Some(true),
            "premise: the prompt is open"
        );

        let mut shown = SHOWN
            .with(Cell::get)
            .expect("the card registered its signal");
        dom.in_runtime(|| shown.set(false));
        // The effect runs as queued work; drive the runtime until the slot
        // reports closed, which its own re-render records.
        while OPEN.with(Cell::get) != Some(false) {
            tokio::time::timeout(std::time::Duration::from_secs(5), dom.wait_for_work())
                .await
                .expect("the hidden-surface effect runs");
            dom.render_immediate(&mut dioxus::core::NoOpMutations);
        }
        dom.mark_dirty(dioxus::core::ScopeId::APP);
        dom.render_immediate(&mut dioxus::core::NoOpMutations);
        assert_eq!(
            HELD.with(Cell::get),
            Some(false),
            "closing the hidden prompt must release its claim"
        );
    }
}
