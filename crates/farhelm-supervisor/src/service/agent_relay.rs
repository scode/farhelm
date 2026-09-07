//! The supervisor's half of the agent→helm relay: an upward request/reply
//! channel bolted onto connections that were, until `PROTOCOL_VERSION` 13,
//! purely helm-drives-supervisor.
//!
//! # Why a proxy exists at all
//!
//! An agent running inside a session is supposed to be able to ask the
//! HELM things — what hosts exist, what sessions exist — because the helm
//! is the thing that knows the fleet and the thing the user is actually
//! looking at. The session cannot ask it directly, and not for want of a
//! nicer API: the host it runs on has no route to the machine the helm
//! runs on, no address for it, and no credential it could present. The
//! arrows all point the other way, by design (SPEC.md: the supervisor
//! listens on no network port; the helm always dials out).
//!
//! What the session DOES have is its own supervisor's unix socket and the
//! per-session credential already injected for `farhelm spawn`. So the
//! supervisor stands in the middle: it accepts
//! [`ControlMsg::AgentRequest`] from a session-authenticated peer and
//! re-sends it up the helm↔supervisor control connection, then relays the
//! answer back down.
//!
//! # The three things that made this more than a forward
//!
//! **Correlation.** Nothing had ever travelled upward as a request, so the
//! supervisor had no notion of "a reply I am waiting for". [`HelmLink`] is
//! that notion, one per connection, with its own `req_id` counter and
//! pending table. The two legs live in SEPARATE `req_id` namespaces — the
//! asking process numbers its own, the supervisor numbers its own — and
//! this module holds the mapping between them for the life of one round
//! trip. Sharing a namespace was never an option: `req_id` has only ever
//! meant "request N on THIS connection".
//!
//! **Routing.** See [`Supervisor::helm_link_for_session`]: the helm that
//! holds the session's attachment is the one asked.
//!
//! **Endings.** A helm can die mid-upcall, and a helm can simply never
//! answer. Both have to end the wait, or the asking `farhelm agent`
//! process hangs forever with nothing to time it out — it has no deadline
//! of its own, deliberately, because the supervisor is the only party that
//! knows which failure actually happened. Connection teardown fails every
//! pending upcall ([`HelmLink::fail_all`]), and an upcall is bounded in two
//! separate stages — see [`HelmLink::upcall`] for why one budget was wrong.
//! The relay can also END a connection ([`HelmLink::retire`]), which is the
//! ending it reaches for when the transport is healthy but the relay on it
//! provably is not.
//!
//! **Mutations end differently from questions.** Once `PROTOCOL_VERSION` 13
//! put `rename`/`stop`/`archive` on this wire, an ending stopped being
//! merely a failure to report and became a claim about whether something
//! happened. A lost connection after the request was queued means "nothing
//! happened, ask again" for a listing and "the outcome is unknown" for a
//! mutation, and the two must not share a sentence — see
//! [`connection_lost_after_queueing`]. The same asymmetry decides how long
//! the delete fence is held: a listing's ends with the call, a mutation's
//! outlives the answer budget, because the budget expiring says nothing
//! about whether the helm is still working. That retention has a last
//! resort of its own — see [`super::core::AGENT_FENCE_RETAIN_TIMEOUT`],
//! which retires the link rather than guessing the mutation ended.

use super::core::{KeyedGuard, Supervisor};
use farhelm_proto::{
    AGENT_MUTATION_UNKNOWN_REMEDY, AGENT_UNAVAILABLE_REMEDY, AgentOutcome, AgentVerb, ControlMsg,
    ErrorKind, Frame,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{Mutex, mpsc, oneshot};
use tracing::warn;

/// What a session is told when no helm holds its attachment, minus the
/// remedy every such refusal shares.
///
/// Named here rather than spelled at the one site that sends it because it
/// is the relay's DEFINING failure — the thing the design accepted in
/// exchange for routing every verb through the helm — and because the e2e
/// test that pins the behavior must match it exactly rather than by
/// substring guesswork. The remedy is appended by [`unavailable`], which is
/// what keeps this ending and the three connection-loss endings from
/// telling a user four different things about the same situation.
pub(crate) const NO_HELM_ATTACHED: &str = "no helm is attached to this session";

/// One full-authority connection's upward request channel.
///
/// Registered when a connection finishes its hello with no session auth
/// (i.e. it is a helm or something else holding full authority), and
/// dropped from the registry when that connection's loop ends.
///
/// The `notify` sender is the connection's ordinary writer queue and
/// doubles as its IDENTITY, which is the same idiom the attachment
/// handlers already use (`ConnectionCtx::tx`'s docs): channel ids are
/// unique only within a connection, so `mpsc::Sender::same_channel` is the
/// only thing that can say "this attachment belongs to that connection".
/// Reusing it here is what lets an attachment be traced back to a link
/// without a second id threaded through every `ActiveAttach` construction.
pub(crate) struct HelmLink {
    /// This connection's writer queue — the outbound half of the link and
    /// its identity. See the struct docs.
    pub(crate) notify: mpsc::Sender<Frame>,
    /// The relay's way to END the connection this link belongs to; see
    /// [`HelmLink::retire`] for the two situations that need one.
    ///
    /// The relay cannot reach the connection any other way. `notify` is the
    /// writer QUEUE, and closing a queue the connection itself holds a
    /// receiver for stops nothing; the read loop is parked in
    /// `read_frame`. So the connection hands its link a signal on the way
    /// up (`Supervisor::register_helm_link`) and selects on the matching
    /// receiver beside its reader.
    shutdown: tokio::sync::watch::Sender<bool>,
    pending: Mutex<Pending>,
    /// The supervisor's own `req_id` counter for THIS connection.
    ///
    /// Per-link, not per-supervisor: `req_id` is scoped to a connection, so
    /// two links may hand out the same number without ambiguity, and a
    /// process-wide counter would only couple unrelated connections.
    /// Starts at 1 — nothing depends on 0 being reserved here, but keeping
    /// the whole protocol's "0 means uncorrelated" convention (see
    /// `ControlMsg::Error`) uniform costs nothing.
    next_req: AtomicU64,
}

/// In-flight upcalls plus the link-is-dead flag under one mutex, for the
/// reason the helm's own `Pending` states: a waiter that observes "not
/// closed", is preempted by the teardown drain, and then inserts, waits on
/// a sender nobody will ever complete.
#[derive(Default)]
struct Pending {
    map: HashMap<u64, PendingUpcall>,
    closed: bool,
}

/// One waiting upcall, and the one thing teardown needs to know about it.
///
/// `mutating` rides along rather than being re-derived because
/// [`HelmLink::fail_all`] no longer has the verb: by the time a connection
/// dies, the request that was sent on it is gone, and the difference
/// between "nothing happened, retry freely" and "this may already have
/// taken effect" is exactly what the asking agent must be told. See
/// [`HelmLink::fail_all`] for why the two endings cannot share one message.
struct PendingUpcall {
    answer: oneshot::Sender<AgentOutcome>,
    /// [`AgentVerb::is_mutating`] for the request this entry is waiting on.
    mutating: bool,
}

impl HelmLink {
    /// Send one [`ControlMsg::AgentRequest`] up this link and await its
    /// answer.
    ///
    /// ## Two budgets, because there are two different failures
    ///
    /// `deliver` bounds getting the frame ONTO the connection's writer
    /// queue; `answer` bounds waiting for the helm once it is there. One
    /// budget over both was the original shape and reported the wrong thing
    /// in the case that matters most: a queue that stayed full for the
    /// whole budget means the helm never received the request, yet the
    /// answer was `Timeout` — whose contract is specifically "it arrived
    /// and may still be running". That inverts the one distinction protocol
    /// 13 exists to make, since an undelivered request is free to retry and
    /// a delivered one is not. Splitting them also stops queue time being
    /// silently deducted from the helm's own budget.
    ///
    /// The queue is bounded and shared with terminal output, so parking
    /// there is ordinary rather than exceptional — which is why the
    /// delivery budget is short: nothing is gained by waiting minutes for
    /// room on a connection that is not draining.
    ///
    /// ## What "delivered" is, exactly, and what `Timeout` therefore means
    ///
    /// `deliver` bounds getting the frame into the connection's writer
    /// QUEUE. `mpsc::Sender::send` resolving means the queue accepted the
    /// item — not that the writer transmitted it, and certainly not that
    /// the helm received it. So the precise reading of a `Timeout` from
    /// this method is: **the request was queued for delivery on the helm's
    /// connection and no answer arrived within the budget; it may or may
    /// not have reached the helm.** A retry is therefore neither provably
    /// free nor provably duplicative, which is exactly what the kind says
    /// and why the message says it in those words.
    ///
    /// The gap is real rather than pedantic. That queue is shared with
    /// terminal traffic, so a connection making slow but steady progress —
    /// enough to stay under its no-progress timeout — can hold an accepted
    /// request behind other frames for longer than the answer budget, and
    /// then transmit it afterwards; the helm does the work and answers into
    /// a pending entry this method has already removed.
    ///
    /// Closing the gap needs a per-frame transmission receipt: the writer
    /// signalling back that THIS frame went out, with the answer budget
    /// starting there and a write failure before it producing `Unavailable`
    /// instead. That is the known refinement and it was deliberately not
    /// built here — the queue is a plain `mpsc::Sender<Frame>` shared by
    /// every attachment site in this supervisor, so a receipt changes that
    /// type everywhere it is constructed. Until then the vocabulary is
    /// honest about its own resolution rather than claiming a delivery it
    /// cannot observe.
    ///
    /// ## Bookkeeping
    ///
    /// The pending entry is registered BEFORE the frame is queued and
    /// removed on every exit path but one: an entry left behind by a caller
    /// that gave up would sit in the map for the connection's whole life,
    /// and a helm answering late would then complete a `oneshot` whose
    /// receiver is gone — harmless, but only by accident. The exception is
    /// a MUTATING verb that ran out its answer budget, where the entry is
    /// deliberately retained; see the section below.
    ///
    /// ## The delete fence outlives the answer budget, for mutations only
    ///
    /// `fence` is the caller's claim on `Supervisor::agent_request_locks`,
    /// held so a `DeleteSession` for the asking session cannot revoke the
    /// credential this mutation was authorized under while it is still in
    /// flight. It is passed in rather than dropped by the caller because
    /// the caller cannot see the moment that matters: the CLI's budget
    /// expiring is not the mutation ending. A budget-shaped release would
    /// free the fence at thirty seconds while the helm is still executing,
    /// which is precisely the window the fence exists to cover.
    ///
    /// So on the answer-timeout path the guard is handed to a task that
    /// holds it until the pending entry resolves one way or the other — a
    /// late [`HelmLink::complete`], or the sender being dropped or
    /// completed by [`HelmLink::fail_all`] when the link dies. Every other
    /// ending drops it immediately, and each is a point at which the
    /// mutation provably cannot still be running: never queued, the queue
    /// gone, or an answer already in hand.
    ///
    /// The hold is therefore bounded by the LINK's life rather than by any
    /// clock, and that is the intended shape rather than a leak: while a
    /// mutation's outcome is genuinely unknown, letting a delete proceed
    /// would be choosing to tear down a session that a still-running
    /// mutation was authorized against — a rename/stop/archive, or a
    /// create/clone that is about to put a real session on some host.
    ///
    /// ## Why the link's life needed a bound of its own
    ///
    /// "Bounded by the link's life" is only a bound if the link can be
    /// counted on to end. It cannot: [`Self::complete`] drops a response
    /// whose `req_id` names no pending entry, which is the right treatment
    /// of an ordinary late answer and is indistinguishable from a helm
    /// answering under an id it has already used. On a connection that
    /// stays healthy, such a retained entry is resolved by nothing, and the
    /// asking session stays fenced against deletion — with each of its
    /// later mutations queued behind the same fence — for the life of the
    /// process.
    ///
    /// So `retain` bounds the retention itself, and its expiry RETIRES THE
    /// LINK (see [`Self::retire`]) rather than releasing the guard. That
    /// distinction is the whole design: dropping the guard on a timer is
    /// the budget-shaped release this method's docs reject, merely with a
    /// longer budget, and it would still be a guess that the mutation
    /// ended. Retiring guesses nothing — it makes connection loss the
    /// terminal event it was always documented to be, so every pending
    /// upcall (this one included) ends as the honest
    /// "delivered, outcome unknown". See
    /// [`super::core::AGENT_FENCE_RETAIN_TIMEOUT`] for the value and why it
    /// is deliberately far past any real mutation.
    ///
    /// `self: &Arc<Self>` exists for that retirement: the retention task
    /// outlives this call and must be able to reach the link.
    async fn upcall(
        self: &Arc<Self>,
        session_id: String,
        request: AgentVerb,
        deliver: std::time::Duration,
        answer: std::time::Duration,
        retain: std::time::Duration,
        fence: Option<KeyedGuard>,
    ) -> AgentOutcome {
        let mutating = request.is_mutating();
        let req_id = self.next_req.fetch_add(1, Ordering::Relaxed);
        let (tx, mut rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            if pending.closed {
                return unavailable("the helm's connection closed before the request was sent");
            }
            pending.map.insert(
                req_id,
                PendingUpcall {
                    answer: tx,
                    mutating,
                },
            );
        }
        let frame = Frame::control(&ControlMsg::AgentRequest {
            req_id,
            session_id,
            request,
        });
        // Whether the pending entry (and, with it, the fence) must outlive
        // this call — see the "delete fence" section above. Only the
        // answer-timeout path on a mutating verb sets it.
        let mut retain_entry = false;
        let outcome = match tokio::time::timeout(deliver, self.notify.send(frame)).await {
            // Queued on the connection. Only now does the helm owe an
            // answer, and only now does its budget start — see this
            // method's docs for why "queued" is as strong as this side can
            // put it.
            //
            // `&mut rx` rather than `rx`: the timeout drops whatever future
            // it was given, and the retention path below needs the receiver
            // to still exist afterwards.
            Ok(Ok(())) => match tokio::time::timeout(answer, &mut rx).await {
                Ok(Ok(outcome)) => outcome,
                // The sender vanished without a value. `fail_all` always
                // sends one, so this is the shape of the whole link being
                // dropped mid-wait — post-queue either way, which is what
                // decides the vocabulary for a mutation.
                Ok(Err(_)) => connection_lost_after_queueing("before it answered", mutating),
                Err(_) => {
                    retain_entry = mutating;
                    answer_budget_expired(answer, mutating)
                }
            },
            // The writer queue is gone: the connection is closing.
            Ok(Err(_)) => unavailable("the helm's connection closed before the request was sent"),
            // Never delivered, so a retry is free — which is exactly what
            // `Unavailable` means and `Timeout` does not.
            Err(_) => unavailable("the request could not be delivered to the helm"),
        };
        if retain_entry {
            // The fence stays claimed until the helm finally answers or the
            // link tears down; both resolve `rx`, and dropping the link
            // resolves it too by dropping the sender. Neither is guaranteed
            // to happen on a live connection, which is what the outer bound
            // is for — see this method's "Why the link's life needed a
            // bound of its own".
            let link = Arc::clone(self);
            tokio::spawn(async move {
                // Held for the whole task, so the guard outlives whichever
                // way the wait below ends.
                let _fence = fence;
                if tokio::time::timeout(retain, rx).await.is_err() {
                    link.retire(&format!(
                        "a mutation went unanswered for {retain:?} past its answer budget, so its \
                         delete fence had no other way to end"
                    ))
                    .await;
                }
            });
            return outcome;
        }
        drop(fence);
        // The map never retains an entry past the wait that owned it,
        // except on the retained path above — see this method's own docs.
        self.pending.lock().await.map.remove(&req_id);
        outcome
    }

    /// Route one inbound [`ControlMsg::AgentResponse`] to whoever is
    /// waiting on it.
    ///
    /// A response naming no pending entry is DROPPED rather than logged as
    /// an error: it is the ordinary shape of a helm answering after the
    /// asking side already timed out or went away, and a warn per late
    /// answer would make a slow helm look like a broken one.
    ///
    /// An id this link NEVER ISSUED is a different thing entirely and gets
    /// a different ending. It cannot be a late answer, because no request
    /// was ever sent under it, so the only readings are a broken helm and a
    /// hostile one — and both mean the answer to some request that WAS sent
    /// is not coming. That matters here more than it would on an ordinary
    /// reply path: an unanswered mutation retains the asking session's
    /// delete fence (see [`Self::upcall`]), and the retention's only
    /// terminal events are an answer and the link dying. Retiring supplies
    /// the second one immediately instead of leaving every fence on this
    /// link to wait out its full bound.
    ///
    /// Ids come from `next_req`, which hands them out before the request is
    /// queued, so "issued" is exactly `< next_req`. `req_id` 0 is included
    /// in the impossible set on purpose: the protocol reserves it for
    /// messages tied to no request at all.
    pub(crate) async fn complete(&self, req_id: u64, outcome: AgentOutcome) {
        if req_id == 0 || req_id >= self.next_req.load(Ordering::Relaxed) {
            self.retire(&format!(
                "the helm answered with req_id {req_id}, which this connection never issued"
            ))
            .await;
            return;
        }
        if let Some(waiter) = self.pending.lock().await.map.remove(&req_id) {
            let _ = waiter.answer.send(outcome);
        }
    }

    /// End this link and the connection under it.
    ///
    /// The relay's own teardown trigger, for the two situations where the
    /// connection is behaving well enough to stay up while the RELAY on it
    /// has provably stopped working: a reply under an id that was never
    /// issued ([`Self::complete`]), and a retained mutation fence that ran
    /// out its last bound ([`Self::upcall`]). Both leave at least one
    /// pending upcall that nothing on a live connection will ever resolve.
    ///
    /// It fails the pending entries HERE rather than leaving that to the
    /// connection's own teardown, so the fence is released at the moment
    /// the decision is made rather than one scheduling hop later, and then
    /// signals the connection so the transport actually closes and the link
    /// leaves the registry. The connection's tail calls
    /// [`Supervisor::unregister_helm_link`], whose `fail_all` is idempotent.
    ///
    /// Loud, at `warn`: unlike the late answers [`Self::complete`] silently
    /// drops, every path here is a protocol violation or a wedged helm, and
    /// killing a connection that may be carrying live terminals is not
    /// something to do without saying why.
    async fn retire(&self, reason: &str) {
        warn!(reason, "retiring a helm connection's agent relay");
        self.fail_all().await;
        // Failure means the connection has already gone; its own teardown
        // is then what unregisters the link.
        let _ = self.shutdown.send(true);
    }

    /// End every upcall this link is carrying, and refuse later ones.
    ///
    /// Called from the connection's teardown tail, and from
    /// [`Self::retire`] when the relay is the party ending things. Without
    /// it a helm dying mid-upcall would leave the asking session waiting
    /// out the full budget for an answer that provably cannot arrive — the
    /// difference between a two-second error and a thirty-second one, for
    /// the failure most likely to actually happen. Idempotent, which is
    /// what lets `retire` fail the entries at the moment of the decision
    /// and the connection's own tail run unchanged behind it.
    ///
    /// A MUTATING verb's entry gets a different ending from a read-only
    /// one's, and the distinction is the point rather than a nicety. Every
    /// entry still in this map has already been accepted by the writer
    /// queue — the pre-queue endings are all handled inside
    /// [`Self::upcall`] itself, which never observes a value deposited here
    /// unless the send succeeded — so the helm may well have received a
    /// mutation (a rename/stop/archive, or a create/clone that puts a new
    /// session on some host), performed it, and lost only the answer. Calling
    /// that `Unavailable`, whose whole contract is "the request was never
    /// delivered and nothing happened", would invite the asking agent to
    /// repeat a mutation that already took effect. `Timeout` is the kind
    /// that already means "delivered, outcome unknown", so that is what a
    /// mutation gets. A listing has nothing to double-apply and keeps the
    /// friendlier, accurate `Unavailable`.
    pub(crate) async fn fail_all(&self) {
        let mut pending = self.pending.lock().await;
        pending.closed = true;
        for (_, waiter) in pending.map.drain() {
            let _ = waiter.answer.send(connection_lost_after_queueing(
                "before it answered",
                waiter.mutating,
            ));
        }
    }
}

/// The ending for a request the helm was given and did not answer inside
/// its budget, in the two vocabularies that situation has.
///
/// The kind is [`ErrorKind::Timeout`] for both classes — the request was
/// queued for delivery either way, which is the whole of what that kind
/// claims — and the shared sentence says so. What a MUTATION adds is the
/// remedy, for the same reason [`connection_lost_after_queueing`] adds it:
/// the neutral "a retry may or may not repeat the request" is an accurate
/// description of the ambiguity and no advice at all, and the two normal
/// post-queue endings of a MUTATION must not answer the "what do
/// I do now?" question differently depending on whether the link died or
/// merely went quiet. A listing keeps the bare sentence, having nothing to
/// double-apply.
fn answer_budget_expired(answer: std::time::Duration, mutating: bool) -> AgentOutcome {
    let shared = format!(
        "the request was queued for delivery to the helm and it did not answer within \
         {answer:?}; it may or may not have reached the helm, and if it did the helm may still \
         be working — so a retry may or may not repeat the request"
    );
    AgentOutcome::Err {
        kind: ErrorKind::Timeout,
        message: if mutating {
            format!("{shared} — {AGENT_MUTATION_UNKNOWN_REMEDY}")
        } else {
            shared
        },
    }
}

/// The ending for a request the writer queue had already accepted when its
/// connection died, in the two vocabularies that situation has.
///
/// `cause` completes the sentence "the helm's connection closed …". For a
/// read-only verb the answer is simply [`unavailable`]: nothing durable was
/// at stake, and the remedy is to make a helm reachable and ask again. For
/// a mutating verb the same facts mean something a caller must act on
/// differently — the mutation may already have happened — so the kind
/// becomes [`ErrorKind::Timeout`] ("delivered, outcome unknown") and the
/// remedy becomes "look before you retry".
fn connection_lost_after_queueing(cause: &str, mutating: bool) -> AgentOutcome {
    if !mutating {
        return unavailable(&format!("the helm's connection closed {cause}"));
    }
    AgentOutcome::Err {
        kind: ErrorKind::Timeout,
        message: format!(
            "the request was queued for delivery to the helm and its connection closed {cause}, \
             so the outcome is unknown — {AGENT_MUTATION_UNKNOWN_REMEDY}"
        ),
    }
}

/// The refusal shape for every "there was nobody to ask, or they went
/// away" ending, so the four sites that produce one cannot drift.
///
/// `cause` says what happened; the remedy is appended here rather than
/// written into each cause. SPEC.md requires a concrete failure to carry an
/// actionable next step, and all four of these endings have the SAME one —
/// no helm is holding this session's attachment right now, and opening the
/// session in a client is what creates one. Three of them used to say only
/// that a connection had closed, which is a fact about the system rather
/// than anything the user can act on.
fn unavailable(cause: &str) -> AgentOutcome {
    AgentOutcome::Err {
        kind: ErrorKind::Unavailable,
        message: format!("{cause} — {AGENT_UNAVAILABLE_REMEDY}"),
    }
}

impl Supervisor {
    /// Register one connection's upward channel; the returned handle is
    /// the connection's own and must be unregistered when its loop ends.
    ///
    /// `shutdown` is how the relay asks that connection to END — see
    /// [`HelmLink::retire`]. It is supplied by the caller rather than
    /// created here because the receiving half has to be selected on beside
    /// the connection's reader, which is a place only the connection can
    /// reach.
    pub(crate) async fn register_helm_link(
        &self,
        notify: mpsc::Sender<Frame>,
        shutdown: tokio::sync::watch::Sender<bool>,
    ) -> Arc<HelmLink> {
        let link = Arc::new(HelmLink {
            notify,
            shutdown,
            pending: Mutex::new(Pending::default()),
            // Starts at 1 — nothing depends on 0 being reserved here, but
            // keeping the whole protocol's "0 means uncorrelated" convention
            // (see `ControlMsg::Error`) uniform costs nothing.
            next_req: AtomicU64::new(1),
        });
        self.helm_links.lock().await.push(Arc::clone(&link));
        link
    }

    /// Drop a connection's link from the registry and fail whatever it was
    /// carrying.
    pub(crate) async fn unregister_helm_link(&self, link: &Arc<HelmLink>) {
        self.helm_links
            .lock()
            .await
            .retain(|registered| !Arc::ptr_eq(registered, link));
        link.fail_all().await;
    }

    /// The link belonging to the helm that holds an attachment to
    /// `session_id`, if any.
    ///
    /// THE ROUTING RULE: "the helm holding this session's attachment". It
    /// is chosen over anything simpler — first registered link, only link,
    /// most recent link — because it is the rule that stays correct if
    /// several helms per supervisor ever become supported (TODO.md's
    /// unbucketized multi-helm entry), and because by construction it
    /// selects the helm the user is actually looking at, which is the whole
    /// mental model this feature is built on.
    ///
    /// What makes it WELL DEFINED is the session-wide LEASE TAKEOVER rule,
    /// not the one-attachment-per-`(session, terminal)` rule. Those are
    /// different invariants and only the first is strong enough here:
    /// per-terminal uniqueness permits one session to have several
    /// attachments at once (its agent pane and a shell tab, say), so on its
    /// own it says nothing about whether those attachments belong to one
    /// client. Lease takeover is what does: claiming a session's lease
    /// displaces the previous owner across that session's terminals, so the
    /// attachments matching a session id are one client's. This function
    /// takes the FIRST match and calls it the owner, which is sound under
    /// that rule and would silently start picking an arbitrary one of two
    /// helms if a refactor preserved only the weaker one.
    ///
    /// Two lock hops rather than one: the attachment identifies its owning
    /// connection only by that connection's writer queue (see
    /// [`HelmLink::notify`]), so the answer is "the registered link whose
    /// queue is the same channel". Both maps are small — attachments are
    /// bounded by live terminals, links by connected helms — and neither
    /// lock is held across the other.
    pub(crate) async fn helm_link_for_session(&self, session_id: &str) -> Option<Arc<HelmLink>> {
        let owner = {
            let attachments = self.attachments.lock().await;
            attachments
                .iter()
                .find(|(key, _)| key.session == session_id)
                .map(|(_, attachment)| attachment.notify.clone())?
        };
        let links = self.helm_links.lock().await;
        links
            .iter()
            .find(|link| link.notify.same_channel(&owner))
            .map(Arc::clone)
    }

    /// Forward one session's request to the helm attached to it and return
    /// the outcome, refusing rather than waiting when there is none.
    ///
    /// The caller has ALREADY authorized the request (that `session_id` is
    /// the connection's own session); this function is the transport, and
    /// deliberately makes no authorization decision of its own so there is
    /// exactly one place holding that rule.
    ///
    /// `fence` is that caller's claim on `Supervisor::agent_request_locks`
    /// — `Some` for a mutating verb, `None` for a listing. Ownership moves
    /// here because only this layer can see when a mutation is really over:
    /// see [`HelmLink::upcall`]'s own docs for why the CLI's answer budget
    /// is the wrong moment to release it.
    pub(crate) async fn relay_agent_request(
        &self,
        session_id: String,
        request: AgentVerb,
        fence: Option<KeyedGuard>,
    ) -> AgentOutcome {
        let Some(link) = self.helm_link_for_session(&session_id).await else {
            // Not a log-worthy event: a session nobody has open in the UI
            // is the ordinary state of most sessions most of the time.
            // Nothing was sent, so the fence has nothing left to protect
            // and is released with this frame.
            return unavailable(NO_HELM_ATTACHED);
        };
        let outcome = link
            .upcall(
                session_id.clone(),
                request,
                self.timeouts.agent_deliver,
                self.timeouts.agent_upcall,
                self.timeouts.agent_fence_retain,
                fence,
            )
            .await;
        if let AgentOutcome::Err {
            kind: ErrorKind::Timeout,
            ..
        } = &outcome
        {
            warn!(
                session = %session_id,
                budget = ?self.timeouts.agent_upcall,
                "the attached helm did not answer an agent request in time"
            );
        }
        outcome
    }
}

#[cfg(test)]
mod tests {
    use super::super::core::KeyedLocks;
    use super::*;
    use farhelm_proto::AgentReply;
    use std::time::Duration;

    /// A stand-in for the retention bound, for the tests that are not about
    /// it.
    ///
    /// Far past anything these tests reach on either clock, so the bound
    /// never fires and never retires a link out from under the property
    /// being tested. Named rather than spelled inline so a reader can see
    /// at each call site that the number is deliberately inert, instead of
    /// having to work out whether it could be reached.
    const RETAIN_FOREVER: Duration = Duration::from_secs(86_400);

    /// A bare link with nothing behind it but a channel, so the endings can
    /// be produced directly rather than by staging a helm that dies.
    ///
    /// The receiver is returned and must be HELD by the caller: dropping it
    /// closes the writer queue, which turns every `upcall` into the
    /// never-sent ending and quietly stops testing anything else. The
    /// shutdown watch's RECEIVER is deliberately dropped: no connection
    /// stands behind these links, so `retire`'s signal has nowhere to go,
    /// and its other half — failing the pending entries — is what these
    /// tests observe.
    fn link() -> (Arc<HelmLink>, mpsc::Receiver<Frame>) {
        let (tx, rx) = mpsc::channel(8);
        let (shutdown, _) = tokio::sync::watch::channel(false);
        (
            Arc::new(HelmLink {
                notify: tx,
                shutdown,
                pending: Mutex::new(Pending::default()),
                next_req: AtomicU64::new(1),
            }),
            rx,
        )
    }

    /// Take the next frame off a link's writer queue and insist it is an
    /// `AgentRequest`, returning its `req_id` and verb.
    ///
    /// Reading the frame is what makes a test's later teardown provably
    /// POST-QUEUE, which is the precondition every mutation-vocabulary claim
    /// rests on. Waiting for the pending entry instead — the shape this
    /// replaces — proves less than it looks: registration happens BEFORE the
    /// send, so a `fail_all` timed against it can land on a request the
    /// queue never accepted, and the endings for those two moments are
    /// deliberately different. Bounded, so a request that never goes out
    /// fails here instead of hanging the suite.
    async fn queued_request(rx: &mut mpsc::Receiver<Frame>) -> (u64, AgentVerb) {
        let frame = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("no request reached the helm's writer queue")
            .expect("the writer queue closed before the request was queued");
        match farhelm_proto::io::parse_control(&frame).expect("decode the queued frame") {
            ControlMsg::AgentRequest {
                req_id, request, ..
            } => (req_id, request),
            other => panic!("expected an AgentRequest on the link, got {other:?}"),
        }
    }

    /// Spec: a connection lost AFTER the request was queued ends a listing
    /// as retry-safe `Unavailable` and a mutation as outcome-unknown
    /// `Timeout`.
    ///
    /// The distinction is the whole point and it is invisible from either
    /// side alone. Both verbs reach exactly the same teardown through
    /// exactly the same code, and the facts are identical — the request was
    /// accepted by the writer queue, the connection then died, nobody knows
    /// what the helm did with it. What differs is the CONSEQUENCE of being
    /// wrong: `Unavailable` tells the asking agent that nothing happened
    /// and it may send the request again, which for a rename/stop/archive
    /// that already took effect is an instruction to apply it twice.
    /// `Timeout`'s documented contract is "delivered, outcome unknown",
    /// which is the truth here.
    ///
    /// Driven through `fail_all` rather than by dropping the link, because
    /// `fail_all` is the path a real connection teardown takes and the one
    /// place the classification is made.
    ///
    /// The teardown is timed against the frame being TAKEN OFF the queue,
    /// not against the pending entry appearing. Those are two different
    /// moments — registration precedes the send — and the whole claim being
    /// tested is about the later one, so a test that only waited for
    /// registration could pass while classifying a request the queue had
    /// never accepted.
    #[farhelm_testtrace::test]
    async fn a_connection_lost_after_queueing_is_unknown_only_for_a_mutation() {
        for (verb, expected) in [
            (AgentVerb::Sessions {}, ErrorKind::Unavailable),
            (AgentVerb::Stop { session_id: None }, ErrorKind::Timeout),
        ] {
            let (link, mut queue) = link();
            let call = tokio::spawn({
                let link = Arc::clone(&link);
                let verb = verb.clone();
                async move {
                    link.upcall(
                        "s1".to_string(),
                        verb,
                        Duration::from_secs(5),
                        Duration::from_secs(30),
                        RETAIN_FOREVER,
                        None,
                    )
                    .await
                }
            });
            let (req_id, queued) = queued_request(&mut queue).await;
            assert_eq!(req_id, 1, "the first upcall on a fresh link is req_id 1");
            assert_eq!(queued, verb, "the queued frame must carry the verb asked");
            link.fail_all().await;

            let AgentOutcome::Err { kind, message } = call.await.expect("the upcall task finishes")
            else {
                panic!("a dead link cannot answer {verb:?} successfully");
            };
            assert_eq!(kind, expected, "{verb:?} ended with the wrong kind");
            if expected == ErrorKind::Timeout {
                assert!(
                    message.contains("outcome is unknown")
                        && message.contains(AGENT_MUTATION_UNKNOWN_REMEDY),
                    "a mutation's ending must say the outcome is unknown and what to do about \
                     it: {message}"
                );
            } else {
                assert!(
                    message.contains(AGENT_UNAVAILABLE_REMEDY),
                    "a listing's ending keeps the retry remedy: {message}"
                );
            }
        }
    }

    /// Spec: a mutation's delete fence outlives the answer budget, and is
    /// released when the helm finally answers — or when the link dies.
    ///
    /// This is the property the fence exists for and the one a reasonable
    /// refactor destroys. Releasing the guard where the `upcall` returns
    /// looks obviously right and is wrong: the CLI's budget expiring is a
    /// statement about how long the asker waited, not about whether the
    /// helm is still executing the mutation, so a delete admitted at that
    /// moment can tear the session down underneath work that is still
    /// running against it. Nothing else observes the difference — the
    /// asking agent gets its `Timeout` either way.
    ///
    /// Both release paths are covered because there are exactly two ways a
    /// retained fence can ever come back, and a hold that only one of them
    /// ended would be a lock leaked for the life of the process on the
    /// other.
    ///
    /// The HOLD is asserted twice and from opposite directions, because the
    /// release half alone does not pin the property: an implementation that
    /// dropped the fence at the answer timeout releases it too, only sooner,
    /// and would satisfy a test that merely waited for the key to clear.
    /// `claimed_for_test` says the key is still held at that instant, and a
    /// second claimer parked on it says the hold is real rather than a
    /// residue in the map — that a delete of the asking session would in
    /// fact have to wait, which is the whole purpose of the fence.
    ///
    /// `start_paused` runs the budget out without spending it: the answer
    /// timeout is real, and waiting thirty seconds for it would make this
    /// the slowest test in the crate.
    #[farhelm_testtrace::test(start_paused = true)]
    async fn a_mutations_fence_outlives_the_answer_budget() {
        for release_by_answer in [true, false] {
            let locks: Arc<KeyedLocks> = Arc::new(KeyedLocks::default());
            let (link, _held_open) = link();
            let outcome = link
                .upcall(
                    "s1".to_string(),
                    AgentVerb::Archive { session_id: None },
                    Duration::from_secs(5),
                    Duration::from_secs(30),
                    RETAIN_FOREVER,
                    Some(locks.claim("s1").await),
                )
                .await;
            let AgentOutcome::Err { kind, message } = &outcome else {
                panic!("an unanswered budget cannot succeed: {outcome:?}");
            };
            assert_eq!(
                *kind,
                ErrorKind::Timeout,
                "the asker is told the budget expired: {outcome:?}"
            );
            // The other normal post-queue ending of a mutation
            // (`connection_lost_after_queueing`) carries this remedy, and a
            // caller cannot tell the two apart from where it sits: both mean
            // "the helm was given this and may have done it". Advice that
            // appeared in only one of them would be advice a caller could
            // not rely on.
            assert!(
                message.contains(AGENT_MUTATION_UNKNOWN_REMEDY),
                "an unanswered mutation must say what to do before retrying: {message}"
            );

            // The defining property: the call has RETURNED and the key is
            // still claimed. Everything after this line is about how the
            // hold ends; this is the assertion that it exists at all.
            assert!(
                locks.claimed_for_test("s1"),
                "the fence must outlive the answer budget, not end with the call"
            );
            // And a real contender proves the hold, rather than a stale map
            // entry with nothing behind it: this is exactly the shape of the
            // `DeleteSession` the fence exists to hold off.
            let contender = tokio::spawn({
                let locks = Arc::clone(&locks);
                async move { locks.claim("s1").await }
            });
            // Its arrival at the lock is observed rather than assumed —
            // see `KeyedLocks::claims_reached_for_test`. This is the second
            // claim of `s1`: the mutation's own was the first.
            //
            // Bounded, because the observation is unbounded by nature: the
            // regression it exists to catch is a claim that was moved or
            // dropped, and against THAT implementation an unbounded await
            // parks the suite forever instead of failing. The bound is a
            // Tokio timer rather than a wall clock so it stays exact under
            // `start_paused` — paused time only advances when the runtime
            // is idle, which here means the contender has been polled and
            // has genuinely not arrived.
            tokio::time::timeout(
                Duration::from_secs(5),
                locks.claims_reached_for_test("s1", 2),
            )
            .await
            .expect("the contender never reached the fence, so it was never claimed");
            assert!(
                !contender.is_finished(),
                "a second claimer must be held off while the mutation's outcome is unknown"
            );

            if release_by_answer {
                link.complete(
                    1,
                    AgentOutcome::Ok {
                        reply: AgentReply::Stopped {},
                    },
                )
                .await;
            } else {
                link.fail_all().await;
            }
            // The contender acquiring is the release: the retention task
            // drops the guard and the key passes straight to the waiter
            // that was blocked on it. Bounded by TURNS rather than by a
            // clock, because `yield_now` keeps the runtime busy and paused
            // time therefore never auto-advances past a deadline, so a
            // release regression fails here naming what did not happen
            // instead of spinning the suite forever.
            let mut turns = 0;
            while !contender.is_finished() {
                turns += 1;
                assert!(
                    turns < 10_000,
                    "the retained fence was never released after the mutation ended by {}",
                    if release_by_answer {
                        "a late answer"
                    } else {
                        "the link dying"
                    }
                );
                tokio::task::yield_now().await;
            }
            // And nothing is left behind once the contender leaves too,
            // which is the map's own half of the contract (see
            // `KeyedGuard`'s `Drop`).
            drop(contender.await.expect("the contender task finishes"));
            assert!(
                !locks.claimed_for_test("s1"),
                "the key must be free once every holder has gone"
            );
        }
    }

    /// Spec: two mutations from ONE asker reach the helm one at a time —
    /// the second is not queued until the first has been answered.
    ///
    /// The fence is documented as protecting a mutation from its own
    /// asker's deletion, and that is what the two handler-side tests pin.
    /// This is its other, unstated consequence, and the one an agent can
    /// actually produce by hand: because the key is the ASKER's session id
    /// rather than the target's, one session's `rename` and `stop` — of
    /// different sessions, even on different hosts — are serialized against
    /// each other. Nothing else in the system enforces an order between
    /// them, so if this ever stopped holding, two lifecycle verbs from one
    /// agent could interleave at the helm in whichever order two upcalls
    /// happened to race, and neither the agent nor the log would show why.
    ///
    /// The claim is taken HERE, the way `handle_restricted_control` takes it
    /// before calling `relay_agent_request` — that the handler really does
    /// take it is a separate fact, pinned by
    /// `handlers::tests::a_mutating_agent_request_claims_the_delete_fence_and_a_listing_does_not`.
    /// What this test owns is the composition: a fence held across a full
    /// upcall keeps the next one off the link entirely, rather than merely
    /// delaying its answer.
    #[farhelm_testtrace::test]
    async fn a_second_mutation_from_one_asker_waits_for_the_first() {
        let locks: Arc<KeyedLocks> = Arc::new(KeyedLocks::default());
        let (link, mut queue) = link();
        let mutation = |verb: AgentVerb| {
            let link = Arc::clone(&link);
            let locks = Arc::clone(&locks);
            async move {
                let fence = locks.claim("asker").await;
                link.upcall(
                    "asker".to_string(),
                    verb,
                    Duration::from_secs(5),
                    Duration::from_secs(30),
                    RETAIN_FOREVER,
                    Some(fence),
                )
                .await
            }
        };

        let first = tokio::spawn(mutation(AgentVerb::Stop { session_id: None }));
        // Its frame being off the queue means the fence is held: the claim
        // happens before the upcall that queued it.
        let (first_id, _) = queued_request(&mut queue).await;

        let second = tokio::spawn(mutation(AgentVerb::Rename {
            session_id: None,
            title: "t".to_string(),
        }));
        // The second task's arrival at the claim, observed rather than
        // assumed — see `KeyedLocks::claims_reached_for_test`. Counting
        // scheduler turns proved nothing: a task that had not been polled at
        // all left the same empty queue as one blocked on the fence, so the
        // assertion below held whether or not the claim was still there.
        // Bounded so that the regression this pins — a second mutation that
        // never takes the fence — fails here rather than parking the suite.
        tokio::time::timeout(
            Duration::from_secs(5),
            locks.claims_reached_for_test("asker", 2),
        )
        .await
        .expect("the second mutation never reached the fence, so nothing serialized it");
        assert!(
            matches!(
                queue.try_recv(),
                Err(tokio::sync::mpsc::error::TryRecvError::Empty)
            ),
            "a second mutation must not be queued while the first is unanswered"
        );

        link.complete(
            first_id,
            AgentOutcome::Ok {
                reply: AgentReply::Stopped {},
            },
        )
        .await;
        assert!(matches!(
            first.await.expect("the first upcall finishes"),
            AgentOutcome::Ok { .. }
        ));

        // Only now, which is what makes the emptiness above an ordering
        // fact rather than a request that was never going to be sent.
        let (second_id, _) = queued_request(&mut queue).await;
        assert_ne!(second_id, first_id, "each upcall takes its own req_id");
        link.complete(
            second_id,
            AgentOutcome::Ok {
                reply: AgentReply::Stopped {},
            },
        )
        .await;
        assert!(matches!(
            second.await.expect("the second upcall finishes"),
            AgentOutcome::Ok { .. }
        ));
    }

    /// Spec: a response under a `req_id` this link NEVER ISSUED retires the
    /// link, while a response under an id it HAS issued and already
    /// finished is dropped and changes nothing.
    ///
    /// Both halves are needed and each is wrong without the other. Dropping
    /// every unrecognized id — the shape this replaces — is unsound on a
    /// connection that stays alive: the asking side has no deadline, the
    /// supervisor's own answer budget only moves a mutation's entry into
    /// retention rather than ending it, so a helm that answers under an
    /// impossible id and never under the real one leaves the asker's delete
    /// fence claimed with nothing left to release it. Retiring EVERY
    /// unrecognized id is the opposite error: a late answer to a request
    /// whose caller gave up is ordinary traffic on a healthy connection,
    /// and killing the connection over it would make a merely slow helm
    /// look like a hostile one.
    ///
    /// The retirement is observed from three sides, because a partial one
    /// is the plausible bug: the in-flight mutation must END (with the
    /// outcome-unknown vocabulary, since its request was queued), its fence
    /// must be RELEASED, and the link must refuse later upcalls rather than
    /// silently accepting requests nobody will answer.
    #[farhelm_testtrace::test]
    async fn a_response_under_a_never_issued_id_retires_the_link() {
        let locks: Arc<KeyedLocks> = Arc::new(KeyedLocks::default());
        let (link, mut queue) = link();

        // One completed round trip, purely so the next phase has an id that
        // provably WAS issued.
        let settled = tokio::spawn({
            let link = Arc::clone(&link);
            async move {
                link.upcall(
                    "s1".to_string(),
                    AgentVerb::Stop { session_id: None },
                    Duration::from_secs(5),
                    Duration::from_secs(30),
                    RETAIN_FOREVER,
                    None,
                )
                .await
            }
        });
        let (settled_id, _) = queued_request(&mut queue).await;
        link.complete(
            settled_id,
            AgentOutcome::Ok {
                reply: AgentReply::Stopped {},
            },
        )
        .await;
        assert!(matches!(
            settled.await.expect("the first upcall finishes"),
            AgentOutcome::Ok { .. }
        ));

        // The benign half: answering it a second time is indistinguishable
        // from a late answer and must leave the link usable.
        link.complete(
            settled_id,
            AgentOutcome::Ok {
                reply: AgentReply::Stopped {},
            },
        )
        .await;

        let mutation = tokio::spawn({
            let link = Arc::clone(&link);
            let fence = locks.claim("s1").await;
            async move {
                link.upcall(
                    "s1".to_string(),
                    AgentVerb::Archive { session_id: None },
                    Duration::from_secs(5),
                    Duration::from_secs(30),
                    RETAIN_FOREVER,
                    Some(fence),
                )
                .await
            }
        });
        // That this reaches the queue at all is the late answer's proof: a
        // link the drop had wrongly retired would refuse it instead.
        let (mutation_id, _) = queued_request(&mut queue).await;

        // The impossible half. `u64::MAX` rather than `mutation_id + 1`
        // because the claim under test is about ids past the counter, and
        // an off-by-one there would be indistinguishable from a real one.
        link.complete(
            u64::MAX,
            AgentOutcome::Ok {
                reply: AgentReply::Stopped {},
            },
        )
        .await;

        let outcome = tokio::time::timeout(Duration::from_secs(5), mutation)
            .await
            .expect("the retirement must end the in-flight mutation, not leave it waiting")
            .expect("the upcall task finishes");
        let AgentOutcome::Err { kind, message } = &outcome else {
            panic!("a retired link cannot answer a mutation successfully: {outcome:?}");
        };
        assert_eq!(
            *kind,
            ErrorKind::Timeout,
            "the request had been queued, so its outcome is unknown: {outcome:?}"
        );
        assert!(
            message.contains(AGENT_MUTATION_UNKNOWN_REMEDY),
            "and the asker must be told what to do before retrying: {message}"
        );
        assert!(
            !locks.claimed_for_test("s1"),
            "the mutation's delete fence must be released once the link is retired"
        );

        // And the link is closed to new work: `mutation_id` was real, so a
        // link that had merely failed this one entry would still accept
        // requests it can no longer get answers for.
        let after = link
            .upcall(
                "s1".to_string(),
                AgentVerb::Sessions {},
                Duration::from_secs(5),
                Duration::from_secs(30),
                RETAIN_FOREVER,
                None,
            )
            .await;
        assert!(
            matches!(
                &after,
                AgentOutcome::Err {
                    kind: ErrorKind::Unavailable,
                    ..
                }
            ),
            "a retired link must refuse rather than queue: {after:?} (mutation was {mutation_id})"
        );
    }

    /// Spec: a retained mutation fence is released — by RETIRING the link —
    /// once the retention's own bound expires.
    ///
    /// This is the last bound on a hold that is otherwise unbounded, and it
    /// exists because "bounded by the link's life" turned out not to be a
    /// bound at all. `complete` drops a response naming no pending entry,
    /// which is correct for a late answer and indistinguishable from a helm
    /// answering under an id it already used; on a connection that never
    /// closes, such a retained entry is resolved by nothing, and the asking
    /// session stays undeletable — with every later mutation of its own
    /// queued behind the same key — for the life of the process.
    ///
    /// What the expiry does is the load-bearing part, and the tempting
    /// simplification is wrong: dropping the guard on a timer is the
    /// budget-shaped release `upcall` argues against with a longer clock,
    /// still guessing that the mutation ended. Retiring guesses nothing, so
    /// this asserts the retirement rather than only the release — the link
    /// must refuse afterwards, which a bare guard-drop would not.
    ///
    /// `start_paused` is what makes a ten-minute production bound testable;
    /// the local `retain` is small only so the arithmetic in this test stays
    /// readable, since paused time costs the same either way.
    #[farhelm_testtrace::test(start_paused = true)]
    async fn a_retained_fence_is_released_when_its_last_bound_expires() {
        let retain = Duration::from_secs(60);
        let locks: Arc<KeyedLocks> = Arc::new(KeyedLocks::default());
        let (link, _held_open) = link();

        let outcome = link
            .upcall(
                "s1".to_string(),
                AgentVerb::Archive { session_id: None },
                Duration::from_secs(5),
                Duration::from_secs(30),
                retain,
                Some(locks.claim("s1").await),
            )
            .await;
        assert!(
            matches!(
                &outcome,
                AgentOutcome::Err {
                    kind: ErrorKind::Timeout,
                    ..
                }
            ),
            "the answer budget expiring is still what the asker is told: {outcome:?}"
        );
        assert!(
            locks.claimed_for_test("s1"),
            "the fence must still be held when the call returns; the bound is the LAST resort"
        );

        // The contender is the delete this fence exists to hold off, and
        // its acquisition below is the only honest evidence of a release.
        let contender = tokio::spawn({
            let locks = Arc::clone(&locks);
            async move { locks.claim("s1").await }
        });
        tokio::time::timeout(
            Duration::from_secs(5),
            locks.claims_reached_for_test("s1", 2),
        )
        .await
        .expect("the contender never reached the fence, so it was never claimed");
        assert!(
            !contender.is_finished(),
            "the fence must still hold a delete off before the bound expires"
        );

        // Explicitly, rather than by letting an idle runtime auto-advance:
        // the point of the test is WHICH deadline fires, so the clock is
        // moved past that one deadline and nothing else.
        tokio::time::advance(retain + Duration::from_secs(1)).await;

        // Turn-bounded rather than clock-bounded for the reason the sibling
        // retention test gives: `yield_now` keeps the runtime busy, so
        // paused time never auto-advances past another deadline while this
        // spins, and a regression fails here naming what did not happen.
        let mut turns = 0;
        while !contender.is_finished() {
            turns += 1;
            assert!(
                turns < 10_000,
                "the retained fence outlived its own bound and was never released"
            );
            tokio::task::yield_now().await;
        }
        drop(contender.await.expect("the contender task finishes"));

        // Retired, not merely released. A version that dropped the guard on
        // the timer and left the link alive passes everything above and
        // fails here, which is the distinction worth having.
        let after = link
            .upcall(
                "s1".to_string(),
                AgentVerb::Sessions {},
                Duration::from_secs(5),
                Duration::from_secs(30),
                retain,
                None,
            )
            .await;
        assert!(
            matches!(
                &after,
                AgentOutcome::Err {
                    kind: ErrorKind::Unavailable,
                    ..
                }
            ),
            "the expiry must retire the link, not just free the key: {after:?}"
        );
    }
}
