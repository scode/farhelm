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

use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;

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
    /// Explicitly drop it where an operation must release before a callback.
    /// Only the guard can release a claim, so an older task cannot clear a
    /// later operation's token after handing ownership back.
    ///
    /// The test-and-set happens synchronously under one write borrow. A
    /// refused claimant constructs no guard and gains no release authority.
    pub(crate) fn claim_guard(&mut self) -> Option<OpGuard> {
        let claimed = claim_in(&mut self.held.write());
        claimed_owner(claimed, || OpGuard { held: self.held })
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
    /// while a Restart is pending (a confirmed Restart prompt hands its claim
    /// there), or by the task a confirmation starts. Each
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

/// One opening of one prompt: the identity an answer must present to act.
///
/// Minted only by [`ConfirmSlot::open`] and [`ConfirmSlots::open`], which
/// number every opening from one process-wide counter and never reuse a
/// number, so two openings of the same prompt (the same `key`, asked again
/// after a cancel, say) still get different tokens, and neither do two slots,
/// or two lanes, with the same key type. `key` is what the opening was for (a
/// tab id, a refused request, a row's question kind, or `()` when the slot
/// holds a single prompt); the opening number is what tells this asking of it
/// apart from every other one.
///
/// Callers cannot construct or alter one: the fields are private to this
/// module, so the only tokens outside it come from `open` or a render's
/// `current` on either type ([`ConfirmSlot::current`],
/// [`ConfirmSlots::current`]), plus the test-only `peek_token` readers. That
/// no handler answers with the token of whatever is open at click time is a
/// convention the `current` docs state, not something the types enforce.
/// A token is good for exactly the opening it names. Once that opening is
/// answered, cancelled, cleared or replaced, the token matches nothing ever
/// again, including a later opening with an equal key.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct ConfirmToken<K> {
    opening: u64,
    key: K,
}

impl<K> ConfirmToken<K> {
    /// What this opening was for, as the code that opened it named it. For
    /// display and for the caller's own checks; the slot compares the whole
    /// token, never the key alone.
    pub(crate) fn key(&self) -> &K {
        &self.key
    }
}

/// One inline "confirm / cancel" prompt: which opening is on screen, and
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
/// it to a running task. The same burst can also hold a cancel and a
/// reopen ahead of a click drawn for the first opening; a check by key alone
/// would let that click answer the second asking, which the user has not
/// seen yet. This type makes the check the only way in, the way [`OpLock`]
/// did for operation exclusion:
///
/// - [`Self::open`] mints a [`ConfirmToken`] for the new opening. The render
///   that draws the prompt reads that token with [`Self::current`] and moves
///   it into each of the prompt's handlers.
/// - [`Self::take`] is the one way a confirm handler learns the user
///   confirmed THIS opening. It is a synchronous test-and-clear: it yields
///   the payload only if the open prompt's token equals the one the handler
///   carries (same opening AND same key), and otherwise changes nothing.
///   (One kind of prompt is excepted; see below the list of things callers
///   must not do.)
/// - [`Self::cancel_for`] closes the prompt only under the same condition;
///   after a `take`, or once another opening has replaced it, it does
///   nothing.
/// - The payload is what the confirmation owns. For the lifecycle prompts it
///   is the [`OpGuard`] claimed when the prompt opened: it lives in the slot
///   while the prompt is open, moves on `take` into whatever the confirmation
///   runs (the confirmed task, or the session view's shared Restart claim), and
///   is dropped (releasing the lock) by cancel, by [`Self::clear`], by
///   replacement, or by the slot itself going away with its component. No
///   handler releases the lock by hand, so none can release someone else's.
///
/// What a caller must not do, because each one reopens the race this type
/// closes:
///
/// - Decide what to answer by reading the live slot in a handler
///   ([`Self::current`] followed by `take`). That answers whatever is open
///   when the click runs, not what the user saw. [`Self::peek_key`] exists
///   for imperative reconciliation (a completion callback retiring a
///   question that no longer describes its draft) and returns no token, so
///   it cannot be turned into an answer.
/// - Keep a token past the render that drew it and answer with it later
///   expecting it to reach a reopened prompt. It will not; that is the point.
/// - Mint or forge a token. Outside this module the fields are private;
///   inside it, only a test builds one by hand, to isolate the key check.
///
/// One exception to "every confirm answer goes through `take`" is
/// sanctioned: a prompt whose surface must stay up through the request its
/// answer starts. The hosts panel's Remove confirmation is the example. Its
/// dialog stays mounted while the removal runs and shows that request's
/// refusal, so a retry after a refusal has to answer the same showing, and a
/// `take` would unmount the dialog and lose both. Such a prompt accepts an
/// answer with [`Self::is_current`] and leaves the opening open. The
/// request's outcome then retires it (the panel calls [`Self::clear`] on
/// success), or the prompt's own cancel does, through [`Self::cancel_for`]
/// or `take` with the token that cancel was drawn for. Because the answer
/// does not consume the opening, two answers queued in one burst can both
/// pass `is_current`; such a prompt needs something else (for host Remove,
/// the request's own claim) to keep the second from starting a second
/// request. The tunnel security warning is another such prompt: its
/// dialog stays up, buttons disabled, while Allow's consent write runs, the
/// write's outcome retires the opening through `take`, and the controller's
/// saving flag refuses a second Allow. The templates editor's "Save these
/// changes before leaving?" prompt is the third: its "save" answer leaves the
/// prompt up through the save and after a failed one, the save's success
/// retires it, and the dialog's busy flag refuses its other answers
/// meanwhile. Every other prompt answers through `take`.
///
/// `Copy` because it is a handle to a signal, like [`OpLock`]. Render code
/// uses the tracked readers ([`Self::is_open`], [`Self::current`]);
/// [`Self::peek_key`], [`Self::is_current`], [`Self::take`] and
/// [`Self::cancel_for`] inspect the live slot without subscribing their
/// caller.
pub(crate) struct ConfirmSlot<K: 'static, P: 'static = ()> {
    state: Signal<SlotState<K, P>>,
}

/// What a [`ConfirmSlot`]'s signal holds: the open prompt, if any.
struct SlotState<K, P> {
    open: Option<(ConfirmToken<K>, P)>,
}

impl<K, P> SlotState<K, P> {
    /// A slot that has never been opened.
    fn closed() -> Self {
        Self { open: None }
    }
}

/// The last opening number handed out, shared by every slot and every lane
/// in the process: [`ConfirmSlot::open`] and [`ConfirmSlots::open`] both
/// mint through [`open_in`]. One counter rather than one per slot, so tokens
/// from two slots with the same key type (the session view keeps several
/// `ConfirmSlot<(), OpGuard>` apart on purpose), or from two lanes of one
/// [`ConfirmSlots`], can never be equal, and a token passed to the wrong
/// slot or lane matches nothing.
static LAST_OPENING: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

impl<K: 'static, P: 'static> Clone for ConfirmSlot<K, P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K: 'static, P: 'static> Copy for ConfirmSlot<K, P> {}

impl<K: 'static, P: 'static> PartialEq for ConfirmSlot<K, P> {
    fn eq(&self, other: &Self) -> bool {
        self.state == other.state
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
        state: use_signal(SlotState::closed),
    }
}

impl<K: PartialEq + Clone + 'static, P: 'static> ConfirmSlot<K, P> {
    /// Open the prompt for `key` as a new opening, holding `payload` until it
    /// is confirmed or dismissed, and return that opening's token.
    ///
    /// An already open prompt is REPLACED (the tab strip moves its close
    /// prompt to the tab clicked last) and its payload dropped; its token,
    /// and any handler still carrying it, stops matching even when `key` is
    /// equal. For a slot whose payload is an operation claim that drop
    /// cannot release anyone else's claim: a second prompt could only have
    /// claimed after the first one's claim was gone.
    ///
    /// Most openers drop the returned token. The answers are drawn by the
    /// next render, which reads the token back through [`Self::current`];
    /// the return value is for code that has no render in between, such as
    /// tests.
    pub(crate) fn open(&mut self, key: K, payload: P) -> ConfirmToken<K> {
        open_in(&mut self.state.write(), key, payload)
    }

    /// Consume the confirmation drawn for `token`: the payload if the open
    /// prompt is exactly that opening, `None` (and nothing changed)
    /// otherwise.
    pub(crate) fn take(&mut self, token: &ConfirmToken<K>) -> Option<P> {
        // Checked through `peek` first so a refused answer does not write
        // the signal: a write would rerender every reader of the slot for
        // a click that changed nothing.
        if !self.is_current(token) {
            return None;
        }
        take_in(&mut self.state.write(), token)
    }

    /// Dismiss the prompt if it is still the opening `token` names, dropping
    /// its payload. Does nothing once a confirmation has taken it or another
    /// opening has replaced it.
    pub(crate) fn cancel_for(&mut self, token: &ConfirmToken<K>) {
        drop(self.take(token));
    }

    /// Close whatever is open, unconditionally. For reconciliation (the
    /// target disappeared, the dialog holding the prompt closed), never for
    /// an answer: an answer must name the opening it was rendered for, which
    /// is what `take` and `cancel_for` require.
    pub(crate) fn clear(&mut self) {
        if self.state.peek().open.is_some() {
            self.state.write().open = None;
        }
    }

    /// Whether any prompt is open, for a RENDER (tracked read).
    pub(crate) fn is_open(&self) -> bool {
        self.state.read().open.is_some()
    }

    /// The open prompt's token, for the RENDER that draws it (tracked read).
    /// Capture it once per render and move it into every handler that render
    /// draws for the prompt; reading it again inside a handler would answer
    /// whatever is open at click time instead.
    pub(crate) fn current(&self) -> Option<ConfirmToken<K>> {
        self.state
            .read()
            .open
            .as_ref()
            .map(|(token, _)| token.clone())
    }

    /// What the open prompt is for, for imperative reconciliation, without
    /// subscribing. A completion callback can run inside another effect; a
    /// tracked read there would make the caller observe this slot's
    /// unrelated later changes. Deliberately the key only, never a token: a
    /// handler that could fetch the live token could answer whatever is open
    /// at click time. Reconciliation that decides the prompt no longer fits
    /// closes it with [`Self::clear`].
    pub(crate) fn peek_key(&self) -> Option<K> {
        self.state
            .peek()
            .open
            .as_ref()
            .map(|(token, _)| token.key.clone())
    }

    /// Whether `token`, the opening a handler's render drew, is still the
    /// open one, without subscribing. For a handler that must decide before
    /// it takes some other claim whether its answer can still land; the
    /// answer itself goes through [`Self::take`], which checks again. The
    /// one exception is a prompt that stays up through the request its
    /// answer starts (the hosts panel's Remove confirmation, the tunnel
    /// security warning, the templates editor's save-before-leaving "save"):
    /// its answer checks `is_current` instead of taking, and the request's
    /// outcome or the prompt's own cancel retires the opening (see the type's
    /// docs for why and what that caller must supply instead). Safe to expose
    /// because the caller already holds the token it asks about: the result
    /// is a yes or no, never the live opening.
    pub(crate) fn is_current(&self, token: &ConfirmToken<K>) -> bool {
        self.state
            .peek()
            .open
            .as_ref()
            .is_some_and(|(open, _)| open == token)
    }

    /// The open prompt's token without subscribing, for tests that need to
    /// name an opening no render drew. Not compiled into the app.
    #[cfg(test)]
    pub(crate) fn peek_token(&self) -> Option<ConfirmToken<K>> {
        self.state
            .peek()
            .open
            .as_ref()
            .map(|(token, _)| token.clone())
    }
}

/// A [`ConfirmSlot`] per lane, for prompts whose openings coexist: one
/// independent slot for each `R` (a sidebar row's session id), all held by
/// the one component that owns the lanes.
///
/// The sidebar rows need this shape and the single slot cannot give it.
/// Each row keeps its own inline Delete or Replace question, several rows
/// can hold one at the same time (opening a question on one row leaves
/// another row's open question alone), and every question must outlive the
/// row's own component: a filtered or failed listing unmounts rows whose
/// questions reappear with them. So the slots live with the list, keyed by
/// row, rather than in the rows.
///
/// Every lane follows [`ConfirmSlot`]'s rules exactly, through the same
/// state, the same test-and-clear and the same process-wide opening
/// counter: [`Self::open`] mints a fresh [`ConfirmToken`] and replaces
/// whatever that lane held; [`Self::take`] and [`Self::cancel_for`] act only
/// for the token of the lane's open opening; a token from an earlier opening,
/// or from another lane, matches nothing. The caller rules on
/// [`ConfirmSlot`] apply per lane, unchanged: read [`Self::current`] in the
/// render and move that token into the handlers it draws; reconcile with
/// [`Self::peek_key`] and [`Self::retain`], never with a token.
///
/// NOTE: `current` being for the render that draws the prompt is a
/// convention, exactly as on [`ConfirmSlot::current`], not something the
/// type enforces. Nothing stops a handler from calling it, and one that did
/// would answer whatever is open at click time instead of what the user saw.
/// The only untracked token reader, [`Self::peek_token`], is compiled for
/// tests only.
///
/// A lane exists only while its prompt is open: an answer, a cancel or
/// reconciliation removes it, so [`Self::open_lanes`] is exactly the set of
/// lanes with a prompt on screen (or retained for a row not drawn now).
pub(crate) struct ConfirmSlots<R: 'static, K: 'static, P: 'static = ()> {
    state: Signal<HashMap<R, SlotState<K, P>>>,
}

impl<R: 'static, K: 'static, P: 'static> Clone for ConfirmSlots<R, K, P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<R: 'static, K: 'static, P: 'static> Copy for ConfirmSlots<R, K, P> {}

/// Create an empty set of lanes. A hook, like [`use_confirm_slot`]: call it
/// unconditionally, once, in the component that owns every lane. What the
/// open lanes hold is dropped when that component unmounts.
pub(crate) fn use_confirm_slots<R: 'static, K: 'static, P: 'static>() -> ConfirmSlots<R, K, P> {
    ConfirmSlots {
        state: use_signal(HashMap::new),
    }
}

impl<R, K, P> ConfirmSlots<R, K, P>
where
    R: Eq + Hash + Clone + 'static,
    K: PartialEq + Clone + 'static,
    P: 'static,
{
    /// Open `lane`'s prompt for `key` as a new opening, holding `payload`,
    /// and return that opening's token. Replaces (and drops) whatever `lane`
    /// held, exactly as [`ConfirmSlot::open`] does; other lanes are untouched.
    /// Most openers drop the token: the next render reads it back through
    /// [`Self::current`].
    pub(crate) fn open(&mut self, lane: R, key: K, payload: P) -> ConfirmToken<K> {
        open_in(
            self.state
                .write()
                .entry(lane)
                .or_insert_with(SlotState::closed),
            key,
            payload,
        )
    }

    /// Consume `lane`'s confirmation drawn for `token`: the payload if the
    /// lane's open prompt is exactly that opening, `None` (and nothing
    /// changed) otherwise.
    pub(crate) fn take<Q>(&mut self, lane: &Q, token: &ConfirmToken<K>) -> Option<P>
    where
        R: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        // Peeked first for `ConfirmSlot::take`'s reason: a refused answer
        // must not write the signal and rerender every reader of the lanes.
        let current = self
            .state
            .peek()
            .get(lane)
            .and_then(|state| state.open.as_ref())
            .is_some_and(|(open, _)| open == token);
        if !current {
            return None;
        }
        let mut lanes = self.state.write();
        let payload = take_in(lanes.get_mut(lane)?, token);
        // The peek above saw `token` open in this lane, so the take succeeded
        // and closed it. Drop the closed lane: a lane exists only while its
        // prompt is open, which is what `open_lanes` relies on.
        lanes.remove(lane);
        payload
    }

    /// Dismiss `lane`'s prompt if it is still the opening `token` names,
    /// dropping its payload. Does nothing once a confirmation has taken it
    /// or another opening has replaced it.
    pub(crate) fn cancel_for<Q>(&mut self, lane: &Q, token: &ConfirmToken<K>)
    where
        R: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        drop(self.take(lane, token));
    }

    /// Close every lane `keep` rejects, unconditionally, dropping what each
    /// held. For reconciliation only (the row a lane belongs to left the
    /// listing), never for an answer: [`ConfirmSlot::clear`] across lanes.
    /// Calls `keep` once per open lane, and writes the signal only when it
    /// closes something, so a listing that changes nothing rerenders nothing.
    pub(crate) fn retain(&mut self, mut keep: impl FnMut(&R) -> bool) {
        let closing: Vec<R> = self
            .state
            .peek()
            .keys()
            .filter(|lane| !keep(lane))
            .cloned()
            .collect();
        if closing.is_empty() {
            return;
        }
        let mut lanes = self.state.write();
        for lane in &closing {
            lanes.remove(lane);
        }
    }

    /// `lane`'s open prompt's token, for the RENDER that draws it (tracked
    /// read). Capture it once per render and move it into every handler that
    /// render draws for the prompt.
    pub(crate) fn current<Q>(&self, lane: &Q) -> Option<ConfirmToken<K>>
    where
        R: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.state
            .read()
            .get(lane)
            .and_then(|state| state.open.as_ref())
            .map(|(token, _)| token.clone())
    }

    /// What `lane`'s open prompt is for, without subscribing: the key only,
    /// never a token, for [`ConfirmSlot::peek_key`]'s reasons. For handlers
    /// that must refuse other work while a lane's prompt is open, and for
    /// reconciliation.
    pub(crate) fn peek_key<Q>(&self, lane: &Q) -> Option<K>
    where
        R: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.state
            .peek()
            .get(lane)
            .and_then(|state| state.open.as_ref())
            .map(|(token, _)| token.key.clone())
    }

    /// Every lane with an open prompt, for a RENDER (tracked read), in no
    /// particular order.
    pub(crate) fn open_lanes(&self) -> Vec<R> {
        self.state.read().keys().cloned().collect()
    }

    /// `lane`'s open token without subscribing, for tests that need to name
    /// an opening no render drew. Not compiled into the app.
    #[cfg(test)]
    pub(crate) fn peek_token<Q>(&self, lane: &Q) -> Option<ConfirmToken<K>>
    where
        R: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.state
            .peek()
            .get(lane)
            .and_then(|state| state.open.as_ref())
            .map(|(token, _)| token.clone())
    }
}

/// [`ConfirmSlot::open`]'s state change over a plain value, split out (like
/// [`claim_in`]) so the rules are testable without a Dioxus runtime. Mints
/// the next opening number from [`LAST_OPENING`]; the counter only ever
/// increases, so no later opening, in this slot or any other, can share a
/// token with an earlier one.
fn open_in<K: Clone, P>(state: &mut SlotState<K, P>, key: K, payload: P) -> ConfirmToken<K> {
    let opening = LAST_OPENING.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    let token = ConfirmToken { opening, key };
    state.open = Some((token.clone(), payload));
    token
}

/// [`ConfirmSlot::take`]'s test-and-clear over a plain value. Taking closes
/// the prompt; it never rewinds the opening numbering.
fn take_in<K: PartialEq, P>(state: &mut SlotState<K, P>, token: &ConfirmToken<K>) -> Option<P> {
    if state.open.as_ref().is_some_and(|(open, _)| open == token) {
        state.open.take().map(|(_, payload)| payload)
    } else {
        None
    }
}

/// The test-and-set itself, over a plain `&mut bool`.
///
/// Split out from [`OpLock::claim_guard`] so the RULE can be exercised without a
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

    /// Imperative question reconciliation must not subscribe its enclosing
    /// effect to later openings. Start with a present question and retain a
    /// tracked positive control, so an empty-slot shortcut cannot hide a read.
    #[farhelm_testtrace::test]
    fn peeking_a_present_confirmation_does_not_subscribe_the_effect() {
        use std::cell::Cell;
        std::thread_local! {
            static SLOT: Cell<Option<ConfirmSlot<u64>>> = const { Cell::new(None) };
            static TRIGGER: Cell<Option<Signal<u64>>> = const { Cell::new(None) };
            static RUNS: Cell<usize> = const { Cell::new(0) };
        }
        /// Mount a present slot before observing it, with one explicit effect
        /// dependency that proves the runtime actually delivers later changes.
        fn app() -> Element {
            let mut slot = use_confirm_slot::<u64, ()>();
            let trigger = use_signal(|| 0_u64);
            use_hook(move || slot.open(1, ()));
            SLOT.with(|value| value.set(Some(slot)));
            TRIGGER.with(|value| value.set(Some(trigger)));
            use_effect(move || {
                let _ = trigger();
                assert!(
                    slot.peek_key().is_some(),
                    "the observed fixture question is present"
                );
                RUNS.with(|value| value.set(value.get() + 1));
            });
            rsx! {}
        }
        RUNS.with(|value| value.set(0));
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        dom.process_events();
        assert_eq!(RUNS.with(Cell::get), 1);
        let mut slot = SLOT.with(Cell::get).unwrap();
        let mut trigger = TRIGGER.with(Cell::get).unwrap();
        dom.in_runtime(|| slot.open(2, ()));
        dom.process_events();
        assert_eq!(
            RUNS.with(Cell::get),
            1,
            "an unrelated later opening is not a dependency"
        );
        dom.in_runtime(|| trigger.set(1));
        dom.process_events();
        assert_eq!(
            RUNS.with(Cell::get),
            2,
            "the effect still follows its intended trigger"
        );
        dom.in_runtime(|| assert_eq!(slot.peek_key(), Some(2)));
    }

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
        let mut slot = SlotState::closed();
        let shown = open_in(&mut slot, "prompt", payload);
        drop(take_in(&mut slot, &shown)); // cancel_for
        assert!(dropped.get(), "cancel releases what the prompt held");
        assert!(
            take_in(&mut slot, &shown).is_none(),
            "a queued confirm finds nothing"
        );

        // confirm, then cancel
        let (payload, dropped) = owned();
        let shown = open_in(&mut slot, "prompt", payload);
        let confirmed = take_in(&mut slot, &shown).expect("the confirm takes the payload");
        drop(take_in(&mut slot, &shown)); // the queued cancel
        assert!(
            !dropped.get(),
            "a queued cancel must not release the confirmed task's claim"
        );
        drop(confirmed);
        assert!(dropped.get());
    }

    /// Why this matters: the second click of a double-click, or a stale
    /// handler rendered for another target, must not act on the prompt that
    /// is open now. Spec: `take` is test-and-clear, so a duplicate confirm
    /// gets nothing; a confirm or cancel whose token names a different key
    /// leaves the open prompt and its payload untouched, even when its
    /// opening number is the open one's (the key is part of the identity,
    /// not decoration).
    #[farhelm_testtrace::test]
    fn duplicate_and_wrong_key_clicks_change_nothing() {
        let (payload, dropped) = owned();
        let mut slot = SlotState::closed();
        let tab_a = open_in(&mut slot, "tab-a", payload);
        // Forged here, where the fields are visible, to isolate the key: the
        // same opening number as the open prompt, for another tab.
        let tab_b = ConfirmToken {
            opening: tab_a.opening,
            key: "tab-b",
        };
        assert!(
            take_in(&mut slot, &tab_b).is_none(),
            "a wrong key takes nothing"
        );
        drop(take_in(&mut slot, &tab_b)); // a cancel rendered for tab-b
        assert!(
            !dropped.get() && slot.open.is_some(),
            "the open prompt survives other keys"
        );
        let first = take_in(&mut slot, &tab_a);
        assert!(first.is_some());
        assert!(
            take_in(&mut slot, &tab_a).is_none(),
            "a duplicate confirm gets nothing"
        );
    }

    /// Why this matters: one burst can hold a Cancel, a click that reopens
    /// the same prompt, and a confirm drawn for the FIRST opening. Keyed by
    /// target alone, that confirm would answer a question the user has not
    /// seen yet (and, for a lifecycle prompt, run the operation with the
    /// claim the new opening took). Spec: a token for an earlier opening of
    /// the same key neither takes nor cancels a reopened prompt, and the
    /// reopened prompt's payload stays held; the reopened prompt's own token
    /// still takes it.
    #[farhelm_testtrace::test]
    fn a_stale_opening_cannot_answer_a_reopened_prompt() {
        let mut slot = SlotState::closed();
        let (payload, _) = owned();
        let first = open_in(&mut slot, (), payload);
        drop(take_in(&mut slot, &first)); // the Cancel
        let (payload, dropped) = owned();
        let reopened = open_in(&mut slot, (), payload);
        assert_eq!(first.key(), reopened.key(), "premise: the same prompt");

        assert!(
            take_in(&mut slot, &first).is_none(),
            "a confirm drawn for the first opening takes nothing"
        );
        drop(take_in(&mut slot, &first)); // a cancel drawn for it
        assert!(
            !dropped.get() && slot.open.is_some(),
            "the reopened prompt and its payload survive the stale answers"
        );
        assert!(
            take_in(&mut slot, &reopened).is_some(),
            "the reopened prompt's own answer still works"
        );

        // Replacement without a close in between is the same case: the tab
        // strip moves its prompt, and the old opening's token goes stale.
        let (payload, _) = owned();
        let replaced = open_in(&mut slot, (), payload);
        let (payload, dropped) = owned();
        let replacing = open_in(&mut slot, (), payload);
        assert!(take_in(&mut slot, &replaced).is_none());
        assert!(!dropped.get(), "the replacing prompt keeps its payload");
        assert!(take_in(&mut slot, &replacing).is_some());
    }

    /// Why this matters: the sidebar rows' Delete and Replace questions live
    /// in one lane per row, and their buttons' clicks can queue behind a
    /// cancel and a reopen of the same row's question in one event burst.
    /// Keyed by lane alone, a confirm drawn for the first showing would
    /// answer the second, which the user has not seen yet; for Delete that
    /// sends a request the user never confirmed in the showing it acts on.
    ///
    /// Spec (`ConfirmSlots`): a token drawn for an earlier opening of a lane
    /// neither confirms (takes) nor cancels a reopened prompt in the same
    /// lane, which stays open with its own token; another lane's open prompt
    /// is independent of both, and its token answers nothing in this lane;
    /// the reopened prompt's own token still answers it and leaves no lane
    /// behind; `retain` closes a lane without a token, after which that
    /// lane's old token matches nothing.
    #[farhelm_testtrace::test]
    fn a_stale_lane_token_answers_no_later_opening_of_that_lane() {
        use std::cell::Cell;
        std::thread_local! {
            static LANES: Cell<Option<ConfirmSlots<String, &'static str>>> =
                const { Cell::new(None) };
        }
        /// Mount a real set of lanes, so the signal writes and lane removal
        /// are the methods the sidebar calls rather than a stand-in.
        fn app() -> Element {
            let lanes = use_confirm_slots::<String, &'static str, ()>();
            LANES.with(|cell| cell.set(Some(lanes)));
            rsx! {}
        }
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        let mut lanes = LANES.with(Cell::get).expect("the lanes mounted");
        dom.in_runtime(|| {
            let first = lanes.open("row-a".to_string(), "delete", ());
            let other = lanes.open("row-b".to_string(), "replace", ());
            lanes.cancel_for("row-a", &first);
            assert_eq!(
                lanes.peek_key("row-a"),
                None,
                "premise: the cancel closed it"
            );
            let reopened = lanes.open("row-a".to_string(), "delete", ());

            assert!(
                lanes.take("row-a", &first).is_none(),
                "a confirm drawn for the first showing takes nothing"
            );
            lanes.cancel_for("row-a", &first);
            assert_eq!(
                lanes.peek_token("row-a").as_ref(),
                Some(&reopened),
                "a cancel drawn for the first showing leaves the reopened prompt"
            );
            assert!(
                lanes.take("row-a", &other).is_none(),
                "another lane's token does not answer this lane"
            );
            assert_eq!(
                lanes.peek_token("row-b").as_ref(),
                Some(&other),
                "this lane's answers leave another lane's prompt alone"
            );
            assert!(
                lanes.take("row-a", &reopened).is_some(),
                "the reopened showing's own answer still works"
            );
            assert_eq!(lanes.peek_key("row-a"), None);
            assert_eq!(
                lanes.open_lanes(),
                vec!["row-b".to_string()],
                "an answered lane leaves nothing behind for the outside-click relays"
            );

            lanes.retain(|id| id != "row-b");
            assert_eq!(
                lanes.peek_key("row-b"),
                None,
                "reconciliation closes a vanished row's lane"
            );
            assert!(lanes.take("row-b", &other).is_none());
        });
    }

    /// Why this matters: a stale token is refused only because its opening
    /// number is never handed out again. A counter that restarted when the
    /// slot emptied, or that `clear` rewound, would mint the old token anew
    /// and let a click drawn for a long-gone prompt answer the next one.
    /// Spec: every `open` on a slot returns a token with a strictly larger
    /// opening than any before it, across take, `clear` and replacement.
    #[farhelm_testtrace::test]
    fn openings_are_never_reused() {
        use std::cell::Cell;
        std::thread_local! {
            static SLOT: Cell<Option<ConfirmSlot<&'static str>>> = const { Cell::new(None) };
        }
        /// Mount a real slot, so `clear` is the method the UI calls rather
        /// than a stand-in over the plain state.
        fn app() -> Element {
            let slot = use_confirm_slot::<&'static str, ()>();
            SLOT.with(|cell| cell.set(Some(slot)));
            rsx! {}
        }
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        let mut slot = SLOT.with(Cell::get).expect("the slot mounted");
        dom.in_runtime(|| {
            let taken = slot.open("prompt", ());
            assert!(slot.take(&taken).is_some());
            let cleared = slot.open("prompt", ());
            slot.clear();
            assert!(slot.peek_key().is_none(), "premise: clear closed it");
            let replaced = slot.open("prompt", ());
            let current = slot.open("prompt", ());
            let openings = [&taken, &cleared, &replaced, &current].map(|token| token.opening);
            assert!(
                openings.windows(2).all(|pair| pair[0] < pair[1]),
                "openings only increase: {openings:?}"
            );
            assert!(slot.is_current(&current));
            for stale in [&taken, &cleared, &replaced] {
                assert!(!slot.is_current(stale), "an earlier opening is not current");
                slot.cancel_for(stale);
                assert_eq!(
                    slot.peek_token().as_ref(),
                    Some(&current),
                    "a token from an earlier opening cannot close the current one"
                );
            }
            assert_eq!(slot.take(&current), Some(()));
            assert_eq!(slot.take(&current), None, "take is test-and-clear");
        });
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
                    let shown = slot.open((), claim);
                    // The confirm click: the claim moves into a task that
                    // stays pending, like a Replace request still in flight.
                    if let Some(claim) = slot.take(&shown) {
                        spawn(async move {
                            let _claim = claim;
                            reached.set(true);
                            std::future::pending::<()>().await;
                        });
                    }
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
        let mut slot = SlotState::closed();
        let shown = open_in(
            &mut slot,
            (),
            claim(&lock).expect("the first prompt claims"),
        );
        assert!(
            claim(&lock).is_none(),
            "a refused claim yields nothing to open a prompt with"
        );

        let task = take_in(&mut slot, &shown).expect("confirmed");
        drop(task); // the confirmed task finishes and releases
        assert!(!lock.get());

        let later_owner = claim(&lock).expect("a later operation claims the lock");
        drop(take_in(&mut slot, &shown)); // a stale cancel for the old prompt
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

        // Model the owner's drop: clearing its cell lets the next operation
        // claim. The public API hands that release authority only to a guard.
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
