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

use super::core::Supervisor;
use farhelm_proto::{
    AGENT_UNAVAILABLE_REMEDY, AgentOutcome, AgentVerb, ControlMsg, ErrorKind, Frame,
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
    map: HashMap<u64, oneshot::Sender<AgentOutcome>>,
    closed: bool,
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
    /// removed on every exit path, including both timeouts: an entry left
    /// behind by a caller that gave up would sit in the map for the
    /// connection's whole life, and a helm answering late would then
    /// complete a `oneshot` whose receiver is gone — harmless, but only by
    /// accident. Removing it explicitly makes the map's size a function of
    /// upcalls in flight and nothing else.
    async fn upcall(
        &self,
        session_id: String,
        request: AgentVerb,
        deliver: std::time::Duration,
        answer: std::time::Duration,
    ) -> AgentOutcome {
        let req_id = self.next_req.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            if pending.closed {
                return unavailable("the helm's connection closed before the request was sent");
            }
            pending.map.insert(req_id, tx);
        }
        let frame = Frame::control(&ControlMsg::AgentRequest {
            req_id,
            session_id,
            request,
        });
        let outcome = match tokio::time::timeout(deliver, self.notify.send(frame)).await {
            // Queued on the connection. Only now does the helm owe an
            // answer, and only now does its budget start — see this
            // method's docs for why "queued" is as strong as this side can
            // put it.
            Ok(Ok(())) => match tokio::time::timeout(answer, rx).await {
                Ok(Ok(outcome)) => outcome,
                Ok(Err(_)) => unavailable("the helm's connection closed before it answered"),
                Err(_) => AgentOutcome::Err {
                    kind: ErrorKind::Timeout,
                    message: format!(
                        "the request was queued for delivery to the helm and it did not answer \
                         within {answer:?}; it may or may not have reached the helm, and if it \
                         did the helm may still be working — so a retry may or may not repeat \
                         the request"
                    ),
                },
            },
            // The writer queue is gone: the connection is closing.
            Ok(Err(_)) => unavailable("the helm's connection closed before the request was sent"),
            // Never delivered, so a retry is free — which is exactly what
            // `Unavailable` means and `Timeout` does not.
            Err(_) => unavailable("the request could not be delivered to the helm"),
        };
        // Unconditional, so the map never retains an entry past the wait
        // that owned it — see this method's own docs.
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
    pub(crate) async fn complete(&self, req_id: u64, outcome: AgentOutcome) {
        if let Some(waiter) = self.pending.lock().await.map.remove(&req_id) {
            let _ = waiter.send(outcome);
        }
    }

    /// End every upcall this link is carrying, and refuse later ones.
    ///
    /// Called from the connection's teardown tail. Without it a helm dying
    /// mid-upcall would leave the asking session waiting out the full
    /// budget for an answer that provably cannot arrive — the difference
    /// between a two-second error and a thirty-second one, for the failure
    /// most likely to actually happen.
    pub(crate) async fn fail_all(&self) {
        let mut pending = self.pending.lock().await;
        pending.closed = true;
        for (_, waiter) in pending.map.drain() {
            let _ = waiter.send(unavailable(
                "the helm's connection closed before it answered",
            ));
        }
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
    pub(crate) async fn register_helm_link(&self, notify: mpsc::Sender<Frame>) -> Arc<HelmLink> {
        let link = Arc::new(HelmLink {
            notify,
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
    pub(crate) async fn relay_agent_request(
        &self,
        session_id: String,
        request: AgentVerb,
    ) -> AgentOutcome {
        let Some(link) = self.helm_link_for_session(&session_id).await else {
            // Not a log-worthy event: a session nobody has open in the UI
            // is the ordinary state of most sessions most of the time.
            return unavailable(NO_HELM_ATTACHED);
        };
        let outcome = link
            .upcall(
                session_id.clone(),
                request,
                self.timeouts.agent_deliver,
                self.timeouts.agent_upcall,
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
