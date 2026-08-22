//! The fleet invalidation feed says something changed without duplicating the
//! changed state.

use std::sync::Arc;
use tokio::sync::watch;

/// The fleet's revision counter: the helm's way of saying "something
/// changed" without saying what (PLAN_M6_75.md item 5).
///
/// One monotonic number behind a `watch`, and every property this type has
/// falls out of that choice:
///
/// - **It carries no data.** A subscriber learns only that the fleet moved,
///   and re-reads whatever its own surface needs through the ordinary REST
///   readers. That is what keeps one payload shape for every kind of change
///   and reuses every consistency rule the REST layer already enforces —
///   the alternative, a typed event per mutation, is a second serving path
///   whose staleness rules would have to be got right twice.
/// - **It coalesces for free.** A `watch` retains only the LATEST value, so
///   a subscriber that was busy through fifty bumps wakes once and re-reads
///   once. A lagged subscriber is therefore not a case to handle; it is the
///   ordinary case observed at a different speed.
/// - **Bumps are CHANGE-only, without exception, by contract of the
///   callers.** Nothing here can tell a real change from a no-op, so every
///   publisher is responsible for calling [`Self::bump`] only when something
///   a client could observe actually differs. That rule is load-bearing
///   rather than tidy: the session-cache refresh runs every few seconds per
///   host and almost always writes an identical row set, so a
///   bump-on-every-refresh feed would wake every open client in the fleet on
///   a three-second timer and be strictly worse than the polling it
///   replaces.
///
///   Every publisher meets it by comparing, not by assuming a mutation
///   changed something: the cache writes compare stored rows inside their
///   own transaction, the actor's publication compares the value it is about
///   to publish, the remembered-default write compares the stored id, and
///   profile edits compare the submitted definition against the catalog
///   before forwarding (`crate::profiles::update_profile`). A caller that
///   cannot tell must not bump.
///
/// Owned by the [`crate::manager::ConnectionManager`] because that is where
/// the chokepoints are — actor state transitions, cache writes, registry
/// reconciliation — and reachable from the REST edge through
/// [`crate::manager::ConnectionManager::events`]
/// for the publishers that live there (profile mutations and
/// remembered-default writes).
pub struct FleetEvents {
    /// Starts at zero and only ever increases. Wrapping is not a case: at
    /// one bump per nanosecond a `u64` lasts ~584 years.
    revision: watch::Sender<u64>,
    /// How many subscriptions are open right now — the admission bound's
    /// only state (see [`Self::admit`]).
    ///
    /// Here rather than in the endpoint because it must be per-HELM: a
    /// process-wide static would make one embedded helm's subscribers count
    /// against another's, which is exactly the sort of coupling a global
    /// makes invisible.
    subscribers: std::sync::atomic::AtomicUsize,
    /// Called at the very start of every [`Self::bump`], before the revision
    /// moves — the seam the publication-ORDER tests observe through.
    ///
    /// Some invariants here are about what a client can see AT the instant of
    /// an invalidation, and the publisher that bumps has, by design, no await
    /// point between settling its state and bumping. That is what makes the
    /// ordering safe and also what makes it invisible to any other task: a
    /// subscriber woken by the bump is polled later, by which time the
    /// ordering it was supposed to prove has already resolved either way. An
    /// observer called synchronously is the only vantage point that can tell
    /// the two orders apart.
    ///
    /// Test-only, so production carries neither the branch nor the mutex.
    #[cfg(test)]
    on_bump: std::sync::Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
}

/// One admitted subscription, releasing its seat when dropped.
///
/// A guard rather than a manual release pair because the release has to
/// survive every way a subscription can end — the peer closing, the write
/// deadline expiring, the task being cancelled during shutdown — and only a
/// destructor covers the last of those.
pub struct FeedSeat {
    events: Arc<FleetEvents>,
}

impl Drop for FeedSeat {
    fn drop(&mut self) {
        self.events
            .subscribers
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }
}

impl Default for FleetEvents {
    fn default() -> FleetEvents {
        FleetEvents::new()
    }
}

impl FleetEvents {
    pub fn new() -> FleetEvents {
        FleetEvents {
            revision: watch::Sender::new(0),
            subscribers: std::sync::atomic::AtomicUsize::new(0),
            #[cfg(test)]
            on_bump: std::sync::Mutex::new(None),
        }
    }

    /// Watch every bump from INSIDE it — see [`Self::on_bump`].
    ///
    /// `observe` must not bump, and must not take any lock a publisher holds
    /// while bumping. It runs on the publisher's own thread, in the middle of
    /// the publisher's work.
    #[cfg(test)]
    pub fn observe_bumps(&self, observe: impl Fn() + Send + Sync + 'static) {
        *self.on_bump.lock().expect("bump observer mutex poisoned") = Some(Box::new(observe));
    }

    /// Take one of `capacity` subscription seats, or `None` when they are
    /// all taken.
    ///
    /// Authentication does not stop one admitted browser from opening sockets
    /// until the helm stops coping; a seat turns that into a refusal the caller
    /// can report. Compare-and-swap rather than check-then-increment, because
    /// two upgrades arriving together must not both see the last free seat.
    ///
    /// `capacity` is the CALLER's, not a constant here: the bound is a
    /// property of the endpoint being protected, and this type is only
    /// counting.
    pub fn admit(self: &Arc<Self>, capacity: usize) -> Option<FeedSeat> {
        self.subscribers
            .fetch_update(
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
                |held| (held < capacity).then_some(held + 1),
            )
            .ok()
            .map(|_| FeedSeat {
                events: Arc::clone(self),
            })
    }

    /// Announce that something a client could observe has changed.
    ///
    /// `send_modify` rather than `send`: a `watch` send is a no-op when
    /// nobody is subscribed, and this channel legitimately has no
    /// subscribers whenever no client is connected. The counter must
    /// advance regardless, or a client that subscribes a moment later would
    /// be handed a revision that silently under-counts what already
    /// happened — and would then sit still until the NEXT change.
    pub fn bump(&self) {
        #[cfg(test)]
        if let Some(observe) = self
            .on_bump
            .lock()
            .expect("bump observer mutex poisoned")
            .as_ref()
        {
            observe();
        }
        self.revision.send_modify(|revision| *revision += 1);
    }

    /// The revision as it stands right now — what a fresh subscriber is
    /// handed immediately, before anything else (see
    /// [`crate::events`]'s handshake).
    ///
    /// ## It can NEVER qualify data in helm.db
    ///
    /// Tempting and wrong: "the revision has not moved, so the count I took
    /// earlier still describes the rows". Every publisher bumps AFTER its
    /// write commits — a cache replacement commits its transaction and then
    /// calls [`Self::bump`] — so there is always a window in which the rows
    /// are new and this number is old. A reader that trusted it would pair a
    /// count from before a create with a page from after it, and nothing in
    /// the reply would look wrong.
    ///
    /// It is also process-local and restarts at zero, so equality across a
    /// helm restart means nothing at all.
    ///
    /// What it CAN qualify is state that lives in this process and is
    /// published under the same discipline — the identity-less hosts'
    /// in-memory session lists, which are only ever changed by a publication
    /// that bumps this. `crate::aggregate` uses it for exactly that half and
    /// takes the store's own generation for the other.
    pub fn revision(&self) -> u64 {
        *self.revision.borrow()
    }

    /// A receiver that yields every later revision, coalesced.
    ///
    /// The receiver starts out marked as having SEEN the current value, so
    /// a caller must read [`Self::revision`] (or `borrow_and_update`) for
    /// the starting point and then wait for changes. Subscribing does not
    /// itself count as a change.
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.revision.subscribe()
    }
}
