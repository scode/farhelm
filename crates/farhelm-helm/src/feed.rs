//! The fleet invalidation feed says something changed without duplicating the
//! changed state.
//!
//! It is in-process only. The UI channel (`crate::ui_channel`) subscribes and
//! re-runs its pushed collections' GET handlers on every bump, and several of
//! the helm's own tasks wait on it. The browser-era `/api/events` WebSocket
//! that once relayed the revision to clients is gone: the maintainer decided
//! on 2026-11-15 to remove it once a search found no caller left in Farhelm.

use tokio::sync::watch;

/// The fleet's revision counter: the helm's way of saying "something
/// changed" without saying what (PLAN_M6_75.md item 5).
///
/// One monotonic number behind a `watch`, and every property this type has
/// falls out of that choice:
///
/// - **It carries no data.** A subscriber learns only that the fleet moved,
///   and re-checks what it cares about. The UI channel, the one listener a
///   client sees, re-reads through the ordinary GET handlers it runs in
///   process; the helm's own waiters (an agent approval's wait, automatic
///   updates) re-check their own state. That is what keeps one payload shape
///   for every kind of change and reuses every consistency rule those
///   handlers already enforce — the alternative, a typed event per mutation,
///   is a second serving path whose staleness rules would have to be got
///   right twice.
/// - **It coalesces for free.** A `watch` retains only the LATEST value, so
///   a subscriber that was busy through fifty bumps wakes once and re-reads
///   once. A lagged subscriber is therefore not a case to handle; it is the
///   ordinary case observed at a different speed.
/// - **Bumps are CHANGE-only, by contract of the callers.** Nothing here
///   can tell a real change from a no-op, so every
///   publisher is responsible for calling [`Self::bump`] only when something
///   a client could observe actually differs. That rule is load-bearing
///   rather than tidy: the session-cache refresh runs every few seconds per
///   host and almost always writes an identical row set, so a
///   bump-on-every-refresh feed would make the UI channel re-run every
///   pushed collection's read on a three-second timer per host, for nothing.
///
///   The frequent publishers meet it by comparing, not by assuming a
///   mutation changed something: the cache writes compare stored rows
///   inside their own transaction, and the actor's publication compares the
///   value it is about to publish. A periodic caller that cannot tell must
///   not bump. A user's own write that the store cannot diff (a preference
///   save, an announcement dismissal) bumps anyway: it happens once per
///   click, never on a timer, and a connected GUI learns of the change only
///   through the feed.
///
/// Owned by the [`crate::manager::ConnectionManager`] because that is where
/// the chokepoints are — actor state transitions, cache writes, registry
/// reconciliation — and reachable from the REST edge through
/// [`crate::manager::ConnectionManager::events`]
/// for the publishers that live there (among them host edits and
/// launch-history writes).
pub struct FleetEvents {
    /// Starts at zero and only ever increases. Wrapping is not a case: at
    /// one bump per nanosecond a `u64` lasts ~584 years.
    revision: watch::Sender<u64>,
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

impl Default for FleetEvents {
    fn default() -> FleetEvents {
        FleetEvents::new()
    }
}

impl FleetEvents {
    pub fn new() -> FleetEvents {
        FleetEvents {
            revision: watch::Sender::new(0),
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

    /// Announce that something a client could observe has changed.
    ///
    /// `send_modify` rather than `send`: a `watch` send is a no-op when
    /// nobody is subscribed, and this channel legitimately has no
    /// subscribers whenever no GUI is connected and no helm task is waiting.
    /// The counter must advance regardless, or [`Self::revision`] would
    /// silently under-count what already happened, and a reader comparing it
    /// before and after a change (the tests that check a bump happened do)
    /// would see nothing move.
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

    /// The revision as it stands right now.
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
    /// The same window exists for state that lives in this process: the
    /// identity-less hosts' in-memory session lists are published first and
    /// bumped after, which is why the aggregate's revision-keyed count cache
    /// was removed (see its test
    /// `a_live_change_before_the_snapshot_is_counted_by_the_reply_that_returns_it`).
    /// So this is an invalidation signal only. No production code reads it;
    /// tests read it to observe that a bump happened.
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

// The counter's own contract and the changed-only rule its publishers keep.
// These lived beside the `/api/events` socket until it was removed; they stay
// because the UI channel re-projects on every bump, so a spurious bump costs
// it a re-run of every pushed collection and a missing one leaves the GUI
// showing old state.
#[cfg(test)]
mod tests {
    use super::FleetEvents;
    use crate::rest_harness;
    use axum::body::to_bytes;
    use axum::http::{Request, StatusCode};
    use std::time::Duration;
    use tower::ServiceExt;

    /// How long a test waits for a bump that should be immediate.
    /// Generous because the failure it reports ("nothing arrived") is not
    /// diagnosed any better by a shorter wait.
    const NOTICE: Duration = Duration::from_secs(10);

    /// Coalescing is the property that makes a lagged subscriber a
    /// non-problem rather than a backlog to bound, so it is pinned directly
    /// on the counter rather than inferred from a subscriber's behavior.
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
    /// The failure this pins is silent: `watch::Sender::send` is a no-op
    /// with no receivers, so a counter published that way would stall
    /// whenever nobody happened to be subscribed, and [`FleetEvents::revision`]
    /// and a later subscriber's starting value would under-count what
    /// already happened.
    #[farhelm_testtrace::test]
    async fn a_bump_with_nobody_listening_still_advances_the_revision() {
        let events = FleetEvents::new();
        events.bump();
        events.bump();
        assert_eq!(events.revision(), 2);
        assert_eq!(
            *events.subscribe().borrow(),
            2,
            "a later subscriber must start from what already happened"
        );
    }

    /// A status flip on a host must reach the feed — the milestone's whole
    /// premise, at its smallest observable size.
    ///
    /// Spec: a committed session-cache change (here, one session's status
    /// changing between two drains of the same host) advances the revision,
    /// so the UI channel's re-projection on that bump reads the new status.
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
                terminal_missing: false,
                screen_yolo: false,
                claude_trust_prompt: false,
                workspace_trust_prompt: false,
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
    /// three-second-per-host timer for the UI channel's re-projection.
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

    /// Equal suffixes on two entries publish independent rows and counts.
    /// A row permutation changes no stored value; changing one entry's row
    /// must still wake clients even while the other entry reports that suffix.
    #[farhelm_testtrace::test]
    async fn equal_suffix_entries_publish_independent_rows_and_changes() {
        let harness = rest_harness::helm_listing(vec![
            rest_harness::session("shared-a", 200),
            rest_harness::session("shared-b", 100),
        ])
        .await;
        let local = rest_harness::local_id(&harness.store).await;
        let rival = harness
            .store
            .add_ssh_host("rival.example", None, None)
            .await
            .unwrap();
        harness.fleet.edit(rival, |script| {
            script.identity = Some("rival-install".into());
            script.sessions = vec![
                rest_harness::session("shared-a", 200),
                rest_harness::session("shared-b", 100),
            ];
        });
        harness.manager.sync_registry().await.unwrap();
        harness.await_refreshed(rival).await;
        for host in [local, rival] {
            assert_eq!(
                harness.store.cached_sessions(host).await.unwrap().len(),
                2,
                "premise: both entries admitted their own rows"
            );
        }
        let events = std::sync::Arc::clone(harness.manager.events());
        let before = events.revision();
        harness
            .fleet
            .edit(rival, |script| script.sessions.reverse());
        harness.refresh_to_completion(rival).await;
        assert_eq!(events.revision(), before, "row order alone is not a change");

        let mut subscriber = events.subscribe();
        subscriber.mark_unchanged();
        harness.fleet.edit(rival, |script| {
            script.sessions[0].title = "renamed rival row".into()
        });
        harness.manager.refresh_now(rival);
        tokio::time::timeout(NOTICE, subscriber.changed())
            .await
            .expect("a changed row on either entry invalidates")
            .expect("the manager outlives this test");
        assert!(events.revision() > before);
        let response = harness
            .router()
            .oneshot(
                Request::builder()
                    .uri("/api/sessions")
                    .header("host", "127.0.0.1:7433")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["total"], 4);
        let mut ids: Vec<_> = value["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["id"].as_str().unwrap().to_owned())
            .collect();
        ids.sort();
        let mut expected: Vec<_> = [local, rival]
            .into_iter()
            .flat_map(|host| ["shared-a", "shared-b"].map(|id| harness.session_ref(host, id)))
            .collect();
        expected.sort();
        assert_eq!(ids, expected);
        let renamed = value["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == harness.session_ref(rival, "shared-b"))
            .unwrap();
        assert_eq!(renamed["title"], "renamed rival row");
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
}
