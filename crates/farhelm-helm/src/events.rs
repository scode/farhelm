//! `/api/events` — the invalidation feed: the socket that says "something
//! changed" and nothing else (PLAN_M6_75.md item 5).
//!
//! This is the channel that replaces the UI's four periodic loops. It
//! carries REVISION NUMBERS ONLY — never a session, never a host, never a
//! diff — and a client that receives one re-reads whatever its current
//! surface needs through the readers it already has (the session list, the
//! detail fetch, the hosts read, the profiles read). Three things fall out
//! of that choice, and they are the whole reason it is shaped this way:
//!
//! - **One payload shape for every kind of change.** A create, a rename, a
//!   status flip, a host going down and a profile edit are indistinguishable
//!   here, so there is exactly one thing to get right on the wire instead of
//!   one per mutation.
//! - **A lagged subscriber collapses to one re-read.** The revision is a
//!   `watch` (see [`crate::manager::FleetEvents`]), which retains only the
//!   latest value, so a client that was busy through fifty changes wakes
//!   once. A backlog is impossible by construction rather than by a bound
//!   somebody has to tune.
//! - **Every consistency rule stays where it already is.** The REST readers
//!   enforce staleness marking, owner routing, the listing cap and the
//!   rest; a push channel carrying data would be a second serving path that
//!   had to enforce all of it again, in a different shape, from a different
//!   moment in time.
//!
//! ## The handshake, and the race it closes
//!
//! On (re)subscription the server sends the CURRENT revision IMMEDIATELY,
//! before waiting for anything. That is not a courtesy — it is what makes
//! the client's fallback-to-feed handover safe (PLAN_M6_75.md item 6). A
//! client that loses the socket falls back to polling; when the socket
//! comes back, a mutation that landed between its last poll and its
//! resubscription would be invisible to both — the poll is over, and the
//! bump happened before the subscription existed. Sending the current
//! revision at once gives the client one guaranteed re-read to hang the
//! handover on: it re-reads on the handshake message, and only then stops
//! polling.
//!
//! For the same reason the handshake message is not distinguishable from a
//! later one, and must not be: the client's correct response to EITHER is
//! "re-read", so a distinct greeting would only invite a client to treat
//! them differently.
//!
//! ## Authenticated, still bounded
//!
//! The API router rejects an invalid device secret before this upgrade is
//! reached, and token rotation closes an admitted feed through the
//! [`crate::auth::AuthenticatedSocket`] carried into its serving task. That
//! boundary is not a substitute for resource limits: one authenticated tab
//! can still open sockets in a loop or stop reading. The subscriber cap,
//! inbound frame/message limits, and write deadline keep those failures
//! bounded without pretending the credential makes clients infallible.

use crate::manager::FleetEvents;
use crate::{AppState, auth::AuthenticatedSocket};
use axum::Extension;
use axum::extract::{State, WebSocketUpgrade, ws};
use axum::response::IntoResponse;
use std::sync::Arc;

/// The JSON one revision notification carries: `{"revision":N}`.
///
/// An object rather than a bare `7`, because a bare number on a WebSocket is
/// a shape with nowhere to grow: the first thing this feed ever needs to say
/// alongside a revision (a server-side hint, a reason) would otherwise be a
/// breaking change for every client. What the message MEANS is still one
/// number — the module docs' "revision numbers only" is about the
/// information, not the encoding. The field name is frozen by item 6's
/// consumer.
fn notification(revision: u64) -> String {
    // Hand-built rather than routed through `serde_json`: the shape is one
    // integer field and cannot fail to serialize, so a `Result` here would
    // be a branch no test could ever take.
    format!("{{\"revision\":{revision}}}")
}

/// The largest frame and message this socket will accept from a client.
///
/// A client has NOTHING to say here (see [`serve_events`]), so any inbound
/// payload at all is already a protocol violation — this bound simply makes
/// the violation cheap: a peer cannot make this process buffer megabytes on
/// the way to being disconnected for sending them. One KiB is far more than
/// the zero bytes the protocol defines and far less than anything worth
/// noticing.
const MAX_CLIENT_FRAME: usize = 1024;

/// How long one revision notification may take to reach a client before its
/// subscription is abandoned.
///
/// A subscriber that has stopped reading otherwise parks this task on a send
/// forever, and while it is parked it is not reading either — so the socket's
/// own close would go unnoticed and the task would outlive the browser tab
/// that owned it. The notification is a few dozen bytes, so any peer that is
/// still there delivers it immediately; ten seconds distinguishes "wedged"
/// from "briefly busy" without inventing a heartbeat this feed does not need.
///
/// Dropping a stalled subscriber costs it nothing it can complain about: the
/// feed carries no state, so a client that reconnects is handed the current
/// revision and re-reads (the handshake in the module docs).
const WRITE_DEADLINE: std::time::Duration = std::time::Duration::from_secs(10);

/// How many clients may hold a subscription at once, across this helm.
///
/// One authenticated browser opening sockets in a loop is enough to make a
/// helm hold tasks it will never free. A bound turns that into a refusal
/// rather than unbounded growth. Sized well above any real client population
/// — a browser holds one per tab, and the desktop app one — so reaching it
/// means something is wrong rather than something is popular.
///
/// The DEFAULT, not the value the endpoint reads: that comes from
/// `AppState::event_subscriber_cap`, so a test can exhaust the bound through
/// real sockets rather than against the counter behind it.
pub(crate) const MAX_SUBSCRIBERS: usize = 64;

/// `GET /api/events` — subscribe to the fleet's revision counter.
///
/// Refused with 503 when the subscriber bound is already met, and the
/// refusal is made BEFORE the upgrade: a socket that is opened only to be
/// closed reads to a browser as a flapping connection it should retry
/// harder, whereas an HTTP status is something the client can see and back
/// off from.
///
/// After device authentication there is nothing else to refuse. The feed
/// takes no parameters, names no session, and touches no host, so every other
/// failure this socket can have is a transport failure (see [`serve_events`]).
pub(crate) async fn events_ws(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthenticatedSocket>,
    upgrade: WebSocketUpgrade,
) -> impl IntoResponse {
    let events = Arc::clone(state.manager.events());
    let capacity = state.event_subscriber_cap;
    // Reserved before the upgrade and held by the serving task, so the seat
    // is released exactly when the subscription ends — including when the
    // task is cancelled.
    let Some(seat) = events.admit(capacity) else {
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            format!(
                "this helm is already serving its maximum of {capacity} event subscriptions; \
                 retry shortly"
            ),
        )
            .into_response();
    };
    upgrade
        .protocols([crate::auth::WS_PROTOCOL])
        // Both bounds, not just the message: a peer can otherwise send one
        // enormous frame or a run of small ones assembled into one message,
        // and only one of those is what `max_message_size` alone stops.
        .max_frame_size(MAX_CLIENT_FRAME)
        .max_message_size(MAX_CLIENT_FRAME)
        .on_upgrade(move |socket| async move {
            serve_events(events, socket, auth).await;
            drop(seat);
        })
        .into_response()
}

/// Deliver revisions to one subscribed client until either end goes away.
///
/// Reads as well as writes, and the read half is not decoration: it is how
/// a browser that navigated away or a tab that was closed is noticed
/// promptly. Without draining the socket, a close frame would sit unread and
/// this task would linger until the next bump happened to fail a write —
/// which, on a quiet fleet, could be a very long time.
///
/// ## The client has no vocabulary here, and saying anything ends the
/// subscription
///
/// This feed is one-directional by design: a client that receives a revision
/// re-reads through REST, so there is nothing for it to send. Data frames
/// are therefore treated as a protocol violation and close the socket rather
/// than being ignored. Ignoring them would leave an ambiguity worth avoiding
/// — a future client sending something meaningful would appear to work while
/// being silently dropped — and would let a peer keep a task alive by
/// talking to it. Control frames (ping/pong) are the
/// exception, because the WebSocket layer answers those itself and they are
/// not the client speaking this protocol.
///
/// ## Neither half can pin this task
///
/// The write is bounded by [`WRITE_DEADLINE`], because a subscriber that has
/// stopped reading would otherwise park this task on a send indefinitely —
/// and a parked task is not reading either, so it would not even notice the
/// socket close. Abandoning such a subscriber is safe precisely because the
/// feed is stateless: whatever it missed is one handshake away.
async fn serve_events(events: Arc<FleetEvents>, socket: ws::WebSocket, auth: AuthenticatedSocket) {
    // `SinkExt` is not imported here: the only send this function makes goes
    // through [`notify`], which owns the write deadline.
    use futures_util::StreamExt;

    let (mut tx, mut rx) = socket.split();
    let mut revisions = events.subscribe();
    // The handshake: the current revision, before anything else, so the
    // client has a re-read to hang its fallback handover on (module docs).
    // `borrow_and_update` rather than a plain borrow so this value counts as
    // SEEN — otherwise the wait below would fire immediately and send the
    // same number twice.
    let current = *revisions.borrow_and_update();
    if !notify(&mut tx, current, &auth).await {
        return;
    }
    loop {
        tokio::select! {
            _ = auth.revoked() => return,
            changed = revisions.changed() => {
                // The sender is the manager's, which lives as long as the
                // process serves; an error here means the helm is being torn
                // down, and there is nothing further to say.
                if changed.is_err() {
                    return;
                }
                let revision = *revisions.borrow_and_update();
                if !notify(&mut tx, revision, &auth).await {
                    return;
                }
            }
            incoming = rx.next() => {
                match incoming {
                    // A data frame is a protocol violation: this feed has no
                    // client vocabulary at all (see this function's docs).
                    Some(Ok(ws::Message::Text(_) | ws::Message::Binary(_))) => {
                        tracing::debug!(
                            "an event subscriber sent data on a one-directional feed; closing it"
                        );
                        return;
                    }
                    // Ping/pong are the transport talking, not the client,
                    // and the WebSocket layer has already answered them.
                    Some(Ok(_)) => {}
                    // A closed or broken socket ends the subscription.
                    None | Some(Err(_)) => return,
                }
            }
        }
    }
}

/// Send one revision, or report that this subscription is over.
///
/// Bounded by [`WRITE_DEADLINE`]: `false` means the peer is gone OR has
/// stopped reading, and the two deserve the same answer — the caller returns,
/// the socket drops, and the seat is released. See [`serve_events`] for why a
/// stalled subscriber must not be waited on.
async fn notify<S>(tx: &mut S, revision: u64, auth: &AuthenticatedSocket) -> bool
where
    S: futures_util::SinkExt<ws::Message> + Unpin,
{
    let sent = tokio::select! {
        biased;
        _ = auth.revoked() => return false,
        sent = tokio::time::timeout(
            WRITE_DEADLINE,
            tx.send(ws::Message::Text(notification(revision).into())),
        ) => sent,
    };
    match sent {
        // The send completed; whether it SUCCEEDED is the same question as
        // whether this subscription continues.
        Ok(result) => result.is_ok(),
        Err(_) => {
            tracing::debug!(
                revision,
                "an event subscriber did not accept a revision within the write deadline; \
                 dropping the subscription"
            );
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::manager::FleetEvents;
    use crate::rest_harness::{self, WsTestClient};
    use std::time::Duration;

    /// How long a test waits for a notification that should be immediate.
    /// Generous because the failure it reports ("nothing arrived") is not
    /// diagnosed any better by a shorter wait.
    const NOTICE: Duration = Duration::from_secs(10);

    /// One text frame off the feed, as its revision number.
    ///
    /// Panics rather than returning an error on anything unexpected: every
    /// caller here is asserting that a notification arrived, and a closed
    /// socket or a binary frame IS the failure, not a case to handle.
    async fn revision(ws: &mut WsTestClient) -> u64 {
        let (opcode, payload) = tokio::time::timeout(NOTICE, ws.recv())
            .await
            .expect("the feed must notify promptly")
            .expect("the feed must not close");
        assert_eq!(opcode, 1, "revisions are text frames");
        let text = String::from_utf8(payload).expect("a revision notice is UTF-8");
        let value: serde_json::Value =
            serde_json::from_str(&text).expect("a revision notice is JSON");
        value
            .get("revision")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_else(|| panic!("a revision notice carries a `revision` number: {text}"))
    }

    /// Coalescing is the property that makes a lagged subscriber a
    /// non-problem rather than a backlog to bound, so it is pinned directly
    /// on the counter rather than inferred from an endpoint test.
    ///
    /// Spec: a subscriber that misses N bumps observes ONE change carrying
    /// the LATEST revision — never N wakeups, and never an intermediate
    /// value that would send it re-reading a state that is already gone.
    #[farhelm_testtrace::test]
    async fn a_lagged_subscriber_collapses_every_missed_bump_into_one_re_read() {
        let events = FleetEvents::new();
        let mut subscriber = events.subscribe();
        assert_eq!(*subscriber.borrow_and_update(), 0);

        for _ in 0..5 {
            events.bump();
        }

        subscriber
            .changed()
            .await
            .expect("the sender outlives this test");
        assert_eq!(
            *subscriber.borrow_and_update(),
            5,
            "a lagged subscriber must see the latest revision, not a queue of every step"
        );
        // And nothing further is owed: the five bumps are accounted for by
        // that one observation.
        assert!(
            tokio::time::timeout(Duration::from_millis(50), subscriber.changed())
                .await
                .is_err(),
            "a collapsed backlog must not leave phantom changes behind"
        );
    }

    /// A bump with NO subscribers must still advance the counter.
    ///
    /// The failure this pins is silent and delayed: `watch::Sender::send`
    /// is a no-op with no receivers, so a counter published that way would
    /// stall whenever no client happened to be connected — and the first
    /// client to arrive afterwards would be handed a revision that
    /// under-counts what already happened, then sit still until the next
    /// change. Every helm spends most of its life with nobody watching.
    #[farhelm_testtrace::test]
    async fn a_bump_with_nobody_listening_still_advances_the_revision() {
        let events = FleetEvents::new();
        events.bump();
        events.bump();
        assert_eq!(events.revision(), 2);
        assert_eq!(
            *events.subscribe().borrow(),
            2,
            "a client subscribing later must be handed what already happened"
        );
    }

    /// A status flip on a host must reach the feed — the milestone's whole
    /// premise, at its smallest observable size.
    ///
    /// Spec: a committed session-cache change (here, one session's status
    /// changing between two drains of the same host) advances the revision,
    /// so a client re-reading on that notification sees the new status.
    #[farhelm_testtrace::test]
    async fn a_status_flip_on_a_host_bumps_the_revision() {
        let harness =
            rest_harness::helm_listing(vec![rest_harness::session("sess-1", 1_700_000_000)]).await;
        let local = rest_harness::local_id(&harness.store).await;
        let events = std::sync::Arc::clone(harness.manager.events());
        let mut subscriber = events.subscribe();
        let before = *subscriber.borrow_and_update();

        harness.fleet.edit(local, |script| {
            script.sessions = vec![farhelm_proto::SessionInfo {
                status: farhelm_proto::SessionStatus::Waiting,
                ..rest_harness::session("sess-1", 1_700_000_000)
            }];
        });
        harness.manager.refresh_now(local);

        tokio::time::timeout(NOTICE, subscriber.changed())
            .await
            .expect("a status flip must invalidate")
            .expect("the manager outlives this test");
        assert!(*subscriber.borrow_and_update() > before);
    }

    /// The changed-only rule, which is what stops this feed from being a
    /// three-second-per-host wakeup timer for every open client.
    ///
    /// Spec: a refresh whose replacement writes an IDENTICAL row set
    /// publishes nothing — not a smaller notification, nothing at all.
    ///
    /// Synchronized on refresh COMPLETION rather than on a timeout, which is
    /// the difference between proving this and merely not disproving it: a
    /// test that waited a fixed moment after the request was SENT would pass
    /// just as happily against a slow refresh whose bump landed afterwards.
    /// See `Harness::refresh_to_completion`.
    #[farhelm_testtrace::test]
    async fn a_no_op_refresh_does_not_bump_the_revision() {
        let harness =
            rest_harness::helm_listing(vec![rest_harness::session("sess-1", 1_700_000_000)]).await;
        let local = rest_harness::local_id(&harness.store).await;
        let events = std::sync::Arc::clone(harness.manager.events());
        let before = events.revision();

        // The list the host serves is untouched, so the drain that follows
        // writes back exactly what is already cached — and this returns only
        // once that write has committed and published.
        harness.refresh_to_completion(local).await;

        assert_eq!(
            events.revision(),
            before,
            "a refresh that changed nothing must not wake a single client"
        );
    }

    /// An identity-less host caches nothing and serves from the manager's
    /// memory instead, so its sessions reach the feed by a DIFFERENT path
    /// than every other host's — and that path must obey the same rule.
    ///
    /// Spec: a no-op refresh of an identity-less host publishes nothing,
    /// and a refresh that changes its in-memory list does. Worth its own
    /// test because the two paths share no code: one compares stored rows
    /// inside a SQLite transaction, the other compares published values in
    /// the actor's status.
    #[farhelm_testtrace::test]
    async fn an_identity_less_host_publishes_under_the_same_changed_only_rule() {
        let harness = rest_harness::FleetBuilder::new()
            .await
            .local(rest_harness::HostScript {
                // No identity: nothing to bind a cache write to, so this
                // host's sessions live only in the actor's memory.
                identity: None,
                sessions: vec![rest_harness::session("live-1", 1_700_000_000)],
                ..rest_harness::HostScript::default()
            })
            .await
            .start()
            .await;
        let local = rest_harness::local_id(&harness.store).await;
        harness.await_refreshed(local).await;
        let events = std::sync::Arc::clone(harness.manager.events());
        let mut subscriber = events.subscribe();
        let before = *subscriber.borrow_and_update();

        // Completion, not receipt — see the persisted variant above.
        harness.refresh_to_completion(local).await;
        assert_eq!(
            events.revision(),
            before,
            "an identity-less host's unchanged drain must not wake clients either"
        );

        harness.fleet.edit(local, |script| {
            script
                .sessions
                .push(rest_harness::session("live-2", 1_700_000_100));
        });
        harness.manager.refresh_now(local);
        tokio::time::timeout(NOTICE, subscriber.changed())
            .await
            .expect("a new session on an identity-less host must invalidate")
            .expect("the manager outlives this test");
        assert!(*subscriber.borrow_and_update() > before);
    }

    /// A CONTESTED-set change with no change in cardinality still
    /// invalidates.
    ///
    /// The contested set is not diagnostic decoration: it decides where
    /// `crate::sessions::resolve_owner` routes an operation, including
    /// refusing to route one at all. So a client holding a stale copy can
    /// send a stop to a host that must not receive it. A same-size SWAP —
    /// one collision resolving as another appears — is the case a
    /// length-or-emptiness comparison misses entirely, which is why it is
    /// the case this test stages.
    #[farhelm_testtrace::test]
    async fn a_same_size_contested_swap_invalidates() {
        // The local host caches both ids FIRST, so the rival registered
        // afterwards is deterministically the one that finds them taken —
        // without that ordering, which host ends up contesting depends on
        // whose drain committed first.
        let harness = rest_harness::helm_listing(vec![
            rest_harness::session("shared-a", 200),
            rest_harness::session("shared-b", 100),
        ])
        .await;
        let rival = harness
            .store
            .add_ssh_host("user@rival", None, None)
            .await
            .expect("register the rival host");
        harness.fleet.edit(rival, |script| {
            script.identity = Some("identity-rival".to_string());
            script.sessions = vec![rest_harness::session("shared-a", 200)];
        });
        harness
            .manager
            .sync_registry()
            .await
            .expect("start the rival's actor");
        harness.await_refreshed(rival).await;
        assert_eq!(
            harness.manager.contested_claimants("shared-a"),
            vec![rival],
            "the later host is the one that finds the id already claimed"
        );

        let events = std::sync::Arc::clone(harness.manager.events());
        let before = events.revision();
        let mut subscriber = events.subscribe();
        subscriber.mark_unchanged();

        // The SWAP: the contesting host stops reporting the id it was
        // contesting and reports a different one the local host also holds.
        // The set goes from {shared-a} to {shared-b} — same size, different
        // routing answer.
        harness.fleet.edit(rival, |script| {
            script.sessions = vec![rest_harness::session("shared-b", 100)];
        });
        harness.manager.refresh_now(rival);

        tokio::time::timeout(NOTICE, subscriber.changed())
            .await
            .expect("a contested-set swap must invalidate")
            .expect("the manager outlives this test");
        assert!(events.revision() > before);
        assert_eq!(
            harness.manager.contested_claimants("shared-a"),
            Vec::<crate::store::HostId>::new(),
            "the resolved collision must be gone from the routing answer"
        );
        assert_eq!(
            harness.manager.contested_claimants("shared-b"),
            vec![rival],
            "and the new one must be in it"
        );
    }

    /// A contested set the peer merely REORDERED is not a change.
    ///
    /// Spec: a host that reports the same colliding session ids in a
    /// different order across two refreshes publishes nothing.
    ///
    /// The set is compared by value to decide whether to wake every client,
    /// and its members arrive in whatever order a peer listed its sessions —
    /// which is peer-controlled and need not be stable. Compared as a
    /// sequence, a supervisor that simply reordered its reply would look like
    /// a routing change on every single refresh tick and wake the whole
    /// fleet on the refresh timer, which is exactly the polling this feed
    /// replaces. Canonicalizing at the source is what makes the comparison
    /// about membership.
    ///
    /// A pure permutation is staged rather than a reorder alongside some
    /// other edit, because anything else would also change the set and prove
    /// nothing about the ordering.
    #[farhelm_testtrace::test]
    async fn an_order_only_contested_permutation_does_not_bump() {
        // The local host claims both ids first, so the rival registered
        // afterwards deterministically contests both of them.
        let harness = rest_harness::helm_listing(vec![
            rest_harness::session("shared-a", 200),
            rest_harness::session("shared-b", 100),
        ])
        .await;
        let rival = harness
            .store
            .add_ssh_host("user@rival", None, None)
            .await
            .expect("register the rival host");
        harness.fleet.edit(rival, |script| {
            script.identity = Some("identity-rival".to_string());
            script.sessions = vec![
                rest_harness::session("shared-a", 200),
                rest_harness::session("shared-b", 100),
            ];
        });
        harness
            .manager
            .sync_registry()
            .await
            .expect("start the rival's actor");
        harness.await_refreshed(rival).await;
        assert_eq!(harness.manager.contested_claimants("shared-a"), vec![rival]);
        assert_eq!(harness.manager.contested_claimants("shared-b"), vec![rival]);

        let events = std::sync::Arc::clone(harness.manager.events());
        let before = events.revision();

        // The SAME two ids, listed the other way round. Creation times are
        // swapped with them so the merged order genuinely changes on the
        // peer's side rather than the reply simply being rewritten.
        harness.fleet.edit(rival, |script| {
            script.sessions = vec![
                rest_harness::session("shared-b", 200),
                rest_harness::session("shared-a", 100),
            ];
        });
        harness.refresh_to_completion(rival).await;

        assert_eq!(
            events.revision(),
            before,
            "the same collisions in a different order are the same collisions"
        );
        assert_eq!(harness.manager.contested_claimants("shared-a"), vec![rival]);
        assert_eq!(harness.manager.contested_claimants("shared-b"), vec![rival]);
    }

    /// A refresh that leaves the contested set alone publishes nothing, so
    /// the fix above is a comparison rather than a bump-on-every-drain.
    #[farhelm_testtrace::test]
    async fn an_unchanged_contested_set_does_not_bump() {
        let harness =
            rest_harness::helm_listing(vec![rest_harness::session("shared-a", 200)]).await;
        let rival = harness
            .store
            .add_ssh_host("user@rival", None, None)
            .await
            .expect("register the rival host");
        harness.fleet.edit(rival, |script| {
            script.identity = Some("identity-rival".to_string());
            script.sessions = vec![rest_harness::session("shared-a", 200)];
        });
        harness
            .manager
            .sync_registry()
            .await
            .expect("start the rival's actor");
        harness.await_refreshed(rival).await;
        assert_eq!(harness.manager.contested_claimants("shared-a"), vec![rival]);

        let events = std::sync::Arc::clone(harness.manager.events());
        let before = events.revision();
        harness.refresh_to_completion(rival).await;
        assert_eq!(
            events.revision(),
            before,
            "a standing collision that is still standing is not news"
        );
    }

    /// Registry shape changes invalidate — and a reconcile that changed
    /// nothing does not.
    ///
    /// Both halves matter. A host appearing or disappearing is exactly what
    /// SPEC.md's always-visible connection state must not need a poll to
    /// notice; and `sync_registry` is called after every host mutation
    /// INCLUDING the ones that failed or were no-ops, so a bump per call
    /// would turn the idempotent-reconcile habit into a stream of spurious
    /// wakeups.
    #[farhelm_testtrace::test]
    async fn registry_shape_changes_invalidate_and_no_op_reconciles_do_not() {
        let harness = rest_harness::idle_helm().await;
        let events = std::sync::Arc::clone(harness.manager.events());
        let mut subscriber = events.subscribe();
        let before = *subscriber.borrow_and_update();

        harness
            .manager
            .sync_registry()
            .await
            .expect("reconciling an unchanged registry");
        assert_eq!(
            events.revision(),
            before,
            "a reconcile that found nothing to do must wake nobody"
        );

        let added = harness
            .store
            .add_ssh_host("user@new", None, None)
            .await
            .expect("register a host");
        harness
            .manager
            .sync_registry()
            .await
            .expect("reconciling after an add");
        let after_add = events.revision();
        assert!(after_add > before, "a new host must appear without a poll");

        assert!(harness.manager.stop_actor(added).await);
        assert!(
            events.revision() > after_add,
            "a removed host must disappear without a poll"
        );
    }

    /// The handshake PLAN_M6_75.md item 6's fallback handover is pinned
    /// against: on subscription the server sends the CURRENT revision at
    /// once, before waiting for anything.
    ///
    /// Without it, a client that reconnects after a mutation has nothing to
    /// re-read on — the bump happened before its subscription existed, and
    /// its fallback poll has already stopped — so the change is invisible
    /// to both mechanisms. This asserts the number arrives AND that it is
    /// the current one rather than a zero placeholder.
    #[farhelm_testtrace::test]
    async fn subscribing_is_answered_with_the_current_revision_immediately() {
        let mut harness = rest_harness::idle_helm().await;
        let events = std::sync::Arc::clone(harness.manager.events());
        // Some history, so a server answering "0" would be visibly wrong
        // rather than accidentally right.
        events.bump();
        events.bump();
        let current = events.revision();
        let addr = harness.serve().await;

        let mut ws = WsTestClient::connect(addr, "/api/events").await;
        assert_eq!(
            revision(&mut ws).await,
            current,
            "a fresh subscription must be handed the revision as it stands"
        );
    }

    /// The feed's actual job: a change reaches an already-subscribed client
    /// without it asking.
    ///
    /// Spec: after the handshake, a bump produces a further notification
    /// carrying the new revision. Deliberately NOT "one notification per
    /// bump" — the revision is a `watch`, so bumps that arrive faster than a
    /// client reads are coalesced into one message carrying the latest value
    /// (that collapse is pinned separately, on the counter itself). What is
    /// asserted here is delivery, and that the number delivered is the
    /// current one.
    #[farhelm_testtrace::test]
    async fn a_bump_notifies_an_already_subscribed_client() {
        let mut harness = rest_harness::idle_helm().await;
        let events = std::sync::Arc::clone(harness.manager.events());
        let addr = harness.serve().await;

        let mut ws = WsTestClient::connect(addr, "/api/events").await;
        let handshake = revision(&mut ws).await;

        events.bump();
        assert_eq!(revision(&mut ws).await, handshake + 1);
    }

    /// A client that sends data on this one-directional feed is
    /// disconnected rather than ignored.
    ///
    /// Two reasons, and the second is why "ignore it" is not good enough even
    /// behind authentication: a future client sending something meaningful
    /// would otherwise appear to work while being silently dropped, and an
    /// admitted peer could keep a task alive by talking to a protocol that
    /// has no listener.
    #[farhelm_testtrace::test]
    async fn a_subscriber_that_sends_data_is_disconnected() {
        let mut harness = rest_harness::idle_helm().await;
        let addr = harness.serve().await;

        let mut ws = WsTestClient::connect(addr, "/api/events").await;
        revision(&mut ws).await;

        ws.send_text(r#"{"type":"hello"}"#).await;
        // The server's answer is to stop: either a close frame or the socket
        // going away, both of which surface here as "no more revisions".
        let after = tokio::time::timeout(NOTICE, ws.recv())
            .await
            .expect("the server must react promptly rather than ignoring the frame");
        assert!(
            !matches!(after, Some((1, _))),
            "a feed that answered a client's data frame with business as usual would be \
             inventing a protocol: {after:?}"
        );
    }

    /// The SEAT COUNTER refuses rather than growing, and releases on drop.
    ///
    /// The narrow half of the bound, pinned on the primitive: admission is a
    /// compare-and-swap, so two upgrades arriving together must not both see
    /// the last free seat, and the seat has to come back however the
    /// subscription ended. The ENDPOINT's use of it is a separate question
    /// with its own test below — this one cannot see whether the route
    /// admits at all.
    #[farhelm_testtrace::test]
    async fn the_subscriber_counter_refuses_at_capacity_and_frees_on_drop() {
        use crate::manager::FleetEvents;

        let events = std::sync::Arc::new(FleetEvents::new());
        let first = events.admit(1).expect("the first subscriber is admitted");
        assert!(
            events.admit(1).is_none(),
            "the bound must refuse rather than grow"
        );
        drop(first);
        assert!(
            events.admit(1).is_some(),
            "a seat must come back when its subscription ends"
        );
    }

    /// The ENDPOINT enforces the bound, and refuses BEFORE the upgrade.
    ///
    /// Spec: with the cap exhausted by real subscriptions, the next
    /// `GET /api/events` is answered 503 as plain HTTP — no 101, no socket —
    /// and closing one subscription admits the next.
    ///
    /// This is the half a counter test cannot reach. A route that never
    /// called `admit` would pass every assertion about the counter while
    /// leaving the endpoint unbounded, which is the only place the bound
    /// matters. And "before the upgrade" is the property worth pinning
    /// rather than merely "refused": a socket that opens and closes reads to
    /// a browser as a flapping connection to retry harder, while a status is
    /// something it can back off from.
    ///
    /// The seat's release is asserted through a real disconnect rather than
    /// by dropping a guard, because the release rides a destructor on a
    /// spawned task and the interesting question is whether the task
    /// actually ends.
    #[farhelm_testtrace::test]
    async fn the_events_endpoint_refuses_past_its_cap_and_admits_again_after_a_close() {
        let mut harness = rest_harness::FleetBuilder::new()
            .await
            .event_subscriber_cap(2)
            .start()
            .await;
        let addr = harness.serve().await;

        // Both seats, taken through the route and handshaked so the
        // subscriptions are genuinely established rather than merely
        // connected.
        let mut first = WsTestClient::connect(addr, "/api/events").await;
        revision(&mut first).await;
        let mut second = WsTestClient::connect(addr, "/api/events").await;
        revision(&mut second).await;

        let refused = WsTestClient::try_connect(addr, "/api/events")
            .await
            .err()
            .expect("the third subscription must be refused");
        assert_eq!(
            refused, 503,
            "the bound must be answered as an HTTP status, not as a socket that opens and closes"
        );

        drop(second);
        // The serving task notices the close and releases its seat, which is
        // not instantaneous — poll until it does rather than sleeping a
        // guessed interval.
        let readmitted = tokio::time::timeout(NOTICE, async {
            loop {
                if let Ok(mut ws) = WsTestClient::try_connect(addr, "/api/events").await {
                    revision(&mut ws).await;
                    return;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await;
        assert!(
            readmitted.is_ok(),
            "a seat must come back when its subscriber goes away"
        );
    }

    /// Both inbound size bounds are enforced, and each is caught BEFORE the
    /// bytes it bounds have been accepted.
    ///
    /// Spec: a frame whose declared length exceeds the frame bound ends the
    /// subscription with none of its payload sent, and a run of
    /// individually-legal fragments ends it as soon as they assemble past the
    /// message bound — before the fragment that would finish the message.
    ///
    /// The "before" is the entire content of this test, and its absence is
    /// what made the shape it replaces vacuous. That version sent each
    /// oversized thing IN FULL and then asserted the socket had closed — but
    /// this feed disconnects on any data frame at all (it has no client
    /// vocabulary), so both halves passed with `max_frame_size` and
    /// `max_message_size` deleted outright. The bounds are not about WHETHER
    /// such a peer is disconnected; they are about what this process buffers
    /// on the way. So the frame case now declares a length and sends no body
    /// (a server enforcing only the message bound has nothing to measure and
    /// waits for megabytes that never come), and the message case never sends
    /// its final fragment (a server enforcing only the frame bound sees
    /// nothing but legal 100-byte frames and waits forever).
    ///
    /// The client is hand-rolled precisely so it can emit what no real client
    /// library will — see `WsTestClient::send_frame_header`.
    #[farhelm_testtrace::test]
    async fn an_oversized_frame_or_message_is_refused_before_its_bytes_arrive() {
        let mut harness = rest_harness::idle_helm().await;
        let addr = harness.serve().await;
        let oversized = 2 * super::MAX_CLIENT_FRAME;

        // A header declaring far more than the frame bound, and NOT ONE byte
        // of payload. Only `max_frame_size` can answer this.
        let mut single = WsTestClient::connect(addr, "/api/events").await;
        revision(&mut single).await;
        single.send_frame_header(1, oversized).await;
        let after = tokio::time::timeout(NOTICE, single.recv()).await.expect(
            "a frame declaring more than the bound must be refused on the declaration alone; \
                 waiting for a body that will never arrive is the buffering this bound exists to \
                 prevent",
        );
        assert!(
            !matches!(after, Some((1, _))),
            "an oversized frame must end the subscription, not be read as a revision: {after:?}"
        );

        // Individually-legal fragments assembling past the message bound: a
        // text frame with `fin` clear, then continuations (opcode 0), and no
        // final fragment at all. Only `max_message_size` can answer this.
        let mut fragmented = WsTestClient::connect(addr, "/api/events").await;
        revision(&mut fragmented).await;
        let chunk = vec![b'x'; 100];
        fragmented.send_frame(false, 1, &chunk).await;
        for _ in 0..(oversized / chunk.len()) {
            fragmented.send_frame(false, 0, &chunk).await;
        }
        let after = tokio::time::timeout(NOTICE, fragmented.recv())
            .await
            .expect(
                "fragments must be refused the moment they assemble past the bound, not when the \
                 peer finally says it is done",
            );
        assert!(
            !matches!(after, Some((1, _))),
            "fragments assembling past the message bound must end the subscription: {after:?}"
        );
    }

    /// A subscriber that has stopped reading is DROPPED at the write
    /// deadline rather than parking this task forever.
    ///
    /// Spec: [`notify`] answers `false` once [`WRITE_DEADLINE`] elapses
    /// against a sink that never accepts, and `true` immediately against one
    /// that does.
    ///
    /// Asserted against the sink rather than through a socket, and on
    /// virtual time rather than real: a wedged peer is exactly a sink whose
    /// send never completes, and the alternative — arranging a real client
    /// to fill its receive window and then waiting ten seconds — would be a
    /// slow test that pinned the operating system's buffering as much as the
    /// deadline. `start_paused` advances the clock the moment nothing is
    /// runnable, so the ten seconds are asserted exactly and cost nothing.
    ///
    /// The writable control beside it is what makes this a deadline rather
    /// than a function that always fails: without it, a `notify` that
    /// returned `false` unconditionally would pass.
    #[farhelm_testtrace::test(start_paused = true)]
    async fn a_subscriber_that_never_accepts_a_write_is_dropped_at_the_deadline() {
        use axum::extract::ws;
        use std::pin::Pin;
        use std::task::{Context, Poll};

        /// A sink that accepts nothing, ever — the wedged browser tab, with
        /// none of the machinery it would take to produce one for real.
        struct NeverReady;

        impl futures_util::Sink<ws::Message> for NeverReady {
            type Error = std::convert::Infallible;

            fn poll_ready(
                self: Pin<&mut Self>,
                _: &mut Context<'_>,
            ) -> Poll<Result<(), Self::Error>> {
                Poll::Pending
            }
            fn start_send(self: Pin<&mut Self>, _: ws::Message) -> Result<(), Self::Error> {
                unreachable!("start_send is only reachable once poll_ready succeeds")
            }
            fn poll_flush(
                self: Pin<&mut Self>,
                _: &mut Context<'_>,
            ) -> Poll<Result<(), Self::Error>> {
                Poll::Pending
            }
            fn poll_close(
                self: Pin<&mut Self>,
                _: &mut Context<'_>,
            ) -> Poll<Result<(), Self::Error>> {
                Poll::Pending
            }
        }

        let harness = rest_harness::idle_helm().await;
        let auth = harness.state.auth.socket_session();
        let started = tokio::time::Instant::now();
        assert!(
            !super::notify(&mut NeverReady, 7, &auth).await,
            "a peer that never accepts the write must be abandoned rather than waited on"
        );
        assert_eq!(
            started.elapsed(),
            super::WRITE_DEADLINE,
            "and abandoned exactly at the deadline"
        );

        let mut writable = futures_util::sink::drain();
        assert!(
            super::notify(&mut writable, 7, &auth).await,
            "a peer that is still reading must keep its subscription"
        );
    }

    /// A revocation already waiting wins even against an immediately writable
    /// sink. Since the handshake and every later revision share `notify`, this
    /// pins the no-send-after-rotation boundary for the whole feed.
    #[farhelm_testtrace::test]
    async fn revocation_wins_every_feed_send() {
        let harness = rest_harness::idle_helm().await;
        let auth = harness.state.auth.socket_session();
        harness.state.auth.rotate().await.unwrap();
        let mut writable = futures_util::sink::drain();
        assert!(!super::notify(&mut writable, 7, &auth).await);
    }

    /// Reconnecting re-handshakes — the same immediate current revision a
    /// first subscription gets, not silence until the next change.
    ///
    /// This is the reconnection half of the fallback race: the mutation the
    /// client missed while it was away is represented by a revision it has
    /// never seen, and the handshake is what delivers it. A server that
    /// only spoke on CHANGE would leave the reconnected client waiting for
    /// an unrelated future event before it noticed.
    #[farhelm_testtrace::test]
    async fn a_reconnecting_client_is_re_handshaked_with_what_it_missed() {
        let mut harness = rest_harness::idle_helm().await;
        let events = std::sync::Arc::clone(harness.manager.events());
        let addr = harness.serve().await;

        let mut first = WsTestClient::connect(addr, "/api/events").await;
        let before = revision(&mut first).await;
        drop(first);

        // The change that lands while nobody is subscribed — exactly the
        // window the handshake exists to cover.
        events.bump();
        let missed = events.revision();
        assert!(missed > before);

        let mut second = WsTestClient::connect(addr, "/api/events").await;
        assert_eq!(
            revision(&mut second).await,
            missed,
            "a resubscription must carry what happened while the client was gone"
        );
    }
}
