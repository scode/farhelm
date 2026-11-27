//! Change hints: telling connected helms "this host's sessions changed, poll
//! now" (`ControlMsg::SessionsChanged`, protocol 33).
//!
//! A hint carries nothing but its arrival. The helm answers one by listing
//! this supervisor's sessions through its ordinary `ListSessions` path, so
//! nothing here has to know what changed, only THAT something a user can
//! see did. The alternative, pushing rows or deltas, was rejected: much of a
//! listed row is derived as the reply is built (tabs, status, the restart
//! offer), so every such field would need its own detector and snapshot, and
//! a missed one would leave the helm wrong with nothing to correct it. The
//! helm still polls as a backstop, but a hint is how it hears about a change
//! promptly, so hints are not lost: see "Where hints go".
//!
//! # Who marks, and who sends
//!
//! Code that has just made a user-visible change calls
//! [`Supervisor::hint_sessions_changed`], which is synchronous and costs one
//! `Notify` permit: it never waits, never takes a lock, and is safe to call
//! from any path, including ones holding supervisor mutexes. The rule for
//! callers is "a value a user can see actually changed", never "a read
//! happened": a `ListSessions` that found nothing new must not hint, or the
//! helm's refresh and the supervisor's hint would feed each other forever.
//! The sampler's internal state (screen tails, comparison counts,
//! provisional readings) is not user-visible and never hints; only a change
//! in what `SessionInfo` would carry does.
//!
//! One task, started by `serve` beside the ticker ([`start_hint_sender`]),
//! turns marks into hints. It sends at most one per
//! [`SESSIONS_CHANGED_MIN_GAP`] (a protocol constant, because the helm holds
//! its hint-driven refreshes to the same gap): marks that land during the
//! gap collapse into the single `Notify` permit and go out together when it
//! ends, so a burst (a mass exit, a reap pass closing several tabs) costs
//! the helm one refresh, not one per change. The trailing edge is kept,
//! never dropped: a change marked during the gap is still hinted, just
//! after it.
//!
//! # Where hints go
//!
//! To every registered [`super::agent_relay::HelmLink`], which means every
//! host connection admitted for the owner. Normally there is one, the
//! helm's host connection, and briefly two while it reconnects. Other
//! full-authority connections (a provisioning probe, a test client) register
//! no link and get no hints, which loses nothing, since none of them shows
//! sessions. Before protocol 73 the supervisor could not tell a helm from
//! any other full-authority client and hinted all of them.
//!
//! Each hint is `try_send` on the connection's ordinary writer queue, behind
//! whatever terminal output is already queued. A hint must never block the
//! supervisor, so nothing ever waits for room in that queue. Instead a hint
//! that finds it full is OWED to that connection
//! ([`super::agent_relay::HelmLink::hint_owed`]) and the sender tries again
//! [`OWED_HINT_RETRY`] later, by its own timer, until one is queued. A full
//! queue used to drop the hint and leave the change to the helm's backstop
//! poll; keeping it owed is what lets the helm rely on hints and poll
//! rarely.
//!
//! The retry period is also the debounce for a connection whose queue keeps
//! filling and draining (a busy terminal over a slow link): while it owes a
//! hint, it gets at most one attempt per period, fresh changes included, so
//! it costs the helm one session-list refresh per period rather than one per
//! [`SESSIONS_CHANGED_MIN_GAP`], at a moment its link is already behind. A
//! healthy connection keeps the gap's pace, and one slow connection never
//! holds back another, since each owes on its own and nothing waits.

use super::core::Supervisor;
use farhelm_proto::{ControlMsg, Frame, SESSIONS_CHANGED_MIN_GAP};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;
use tokio::sync::mpsc::error::TrySendError;
use tokio::time::Instant;

/// How long a connection that owes a hint waits between attempts to queue
/// it; see the module docs, "Where hints go".
///
/// Two seconds, the maintainer's choice (2026-11-26): long enough that a
/// queue teetering at its limit cannot turn every change into a refresh at
/// the hint gap's pace, short enough that an owed hint still lands promptly
/// next to the helm's backstop poll.
pub(crate) const OWED_HINT_RETRY: Duration = Duration::from_secs(2);

/// The pending-hint flag the whole supervisor shares: marked by any path
/// that changed something user-visible, drained by the sender task.
///
/// A `Notify` because its single stored permit is exactly the coalescing
/// wanted: any number of marks while nobody is waiting become one wakeup.
#[derive(Debug, Default)]
pub(crate) struct ChangeHints {
    pending: Notify,
}

impl ChangeHints {
    /// Record that a hint is owed. Never blocks.
    pub(crate) fn mark(&self) {
        self.pending.notify_one();
    }
}

impl Supervisor {
    /// Owe connected helms a "sessions changed" hint: something a user can
    /// see about this host's sessions has just changed. Synchronous and
    /// lock-free; see this module's docs for when to call it and when not
    /// to.
    pub(crate) fn hint_sessions_changed(&self) {
        self.change_hints.mark();
    }

    /// Mirror an outcome the store just committed for `entry` into its
    /// in-memory cell, hinting connected helms when that changes it.
    ///
    /// Every path that records an observed exit or error (the timer, wake
    /// or lifecycle operation) mirrors through here, because
    /// whichever of them commits a change first is the only one that sees
    /// it: the others then find the cell already current and have nothing
    /// to compare. A commit that returns the value already held is not a
    /// change and does not hint.
    pub(crate) fn mirror_committed_outcome(
        &self,
        entry: &super::core::SessionEntry,
        committed: &crate::store::LastOutcome,
    ) {
        let changed = {
            let mut outcome = entry.run.outcome.lock().expect("outcome mutex poisoned");
            let changed = *outcome != *committed;
            *outcome = committed.clone();
            if matches!(committed, crate::store::LastOutcome::Error { .. }) {
                // Durable Error now supplies the detail; clear only then so a
                // concurrent stop or failed write cannot erase the observation.
                *entry
                    .run
                    .launch_error
                    .lock()
                    .expect("launch-error mutex poisoned") = None;
            }
            changed
        };
        if changed {
            self.hint_sessions_changed();
        }
    }

    /// Try to queue one `SessionsChanged` on the helm connections that need
    /// one, and return when the earliest connection still owing one should
    /// be tried again (`None` when none owes).
    ///
    /// `fresh` is whether something new was marked. A fresh mark is for every
    /// connection; a timer wake with nothing new is only for the ones that
    /// still owe, since the others already have the hint that change asked
    /// for. Either way a connection that owes is skipped until
    /// [`OWED_HINT_RETRY`] has passed since its last failed attempt; it stays
    /// owed, so the skipped mark is not lost, only folded into the hint it
    /// already owes. A full queue makes a connection owe; a closed one is a
    /// connection on its way out, and owes nothing.
    async fn send_sessions_changed(&self, fresh: bool) -> Option<Instant> {
        let frame = Frame::control(&ControlMsg::SessionsChanged);
        let links = self.helm_links.lock().await;
        // Read after the lock: admission can hold it across store I/O, and a
        // stale instant would shorten the retry period it is compared with.
        let now = Instant::now();
        let mut retry_at: Option<Instant> = None;
        for link in links.iter() {
            let mut owed = link.hint_owed.lock().expect("owed hint mutex poisoned");
            let due = match *owed {
                Some(failed_at) => now.saturating_duration_since(failed_at) >= OWED_HINT_RETRY,
                None => fresh,
            };
            if due {
                *owed = match link.notify.try_send(frame.clone()) {
                    Err(TrySendError::Full(_)) => Some(now),
                    Ok(()) | Err(TrySendError::Closed(_)) => None,
                };
            }
            if let Some(failed_at) = *owed {
                let at = failed_at + OWED_HINT_RETRY;
                retry_at = Some(retry_at.map_or(at, |earliest| earliest.min(at)));
            }
        }
        retry_at
    }
}

/// Owns the hint sender task; dropping it stops the task.
///
/// Held by `serve` for as long as it serves, like the ticker's handle.
pub(crate) struct HintSender {
    task: tokio::task::JoinHandle<()>,
}

impl Drop for HintSender {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Start the task that turns marks into hints; see this module's docs.
///
/// Wakes on a mark, or, while some connection owes a hint, at the earliest
/// moment one is due to be tried again. Holds the supervisor only weakly
/// between hints, so the task never keeps a supervisor alive that everything
/// else has let go of.
pub(crate) fn start_hint_sender(sup: &Arc<Supervisor>) -> HintSender {
    let hints = Arc::clone(&sup.change_hints);
    let sup = Arc::downgrade(sup);
    let task = tokio::spawn(async move {
        let mut retry_at: Option<Instant> = None;
        loop {
            // A mark is consumed only when this arm wins; a retry wake leaves
            // a mark that arrives meanwhile in the `Notify` permit for the
            // next turn, so nothing marked is ever lost.
            let fresh = match retry_at {
                // Biased toward a mark: when a mark and a due retry are both
                // ready, the fresh pass serves both (a due owing connection
                // is tried on any pass). Taking the timer first would send
                // the owing connection its hint now and again on the mark's
                // pass, and delay the healthy connections' hint by a gap.
                Some(at) => tokio::select! {
                    biased;
                    () = hints.pending.notified() => true,
                    () = tokio::time::sleep_until(at) => false,
                },
                None => {
                    hints.pending.notified().await;
                    true
                }
            };
            let Some(sup) = sup.upgrade() else {
                return;
            };
            retry_at = sup.send_sessions_changed(fresh).await;
            drop(sup);
            // sleep-ok: the minimum gap between hints, which is what coalesces a burst
            tokio::time::sleep(SESSIONS_CHANGED_MIN_GAP).await;
        }
    });
    HintSender { task }
}

/// A test's view of the hints a supervisor sends, for tests elsewhere in the
/// service that pin a mutation path's hint without going through `serve`.
#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use std::time::Duration;

    /// A registered helm link plus a running sender: what `serve`
    /// would set up for a connected helm, minus the socket.
    pub(crate) struct HintProbe {
        receiver: tokio::sync::mpsc::Receiver<Frame>,
        _sender: HintSender,
    }

    impl HintProbe {
        pub(crate) async fn attach(sup: &Arc<Supervisor>) -> Self {
            let (notify, receiver) = tokio::sync::mpsc::channel(8);
            let (shutdown, _) = tokio::sync::watch::channel(false);
            let _ = sup.register_helm_link(notify, shutdown).await;
            HintProbe {
                receiver,
                _sender: start_hint_sender(sup),
            }
        }

        /// Wait for the next frame, failing with `what` unless it is a
        /// `SessionsChanged` that arrives within five seconds. Nothing but
        /// the sender writes to this link, so any frame at all is the hint.
        pub(crate) async fn expect_hint(&mut self, what: &str) {
            let frame = tokio::time::timeout(Duration::from_secs(5), self.receiver.recv())
                .await
                .unwrap_or_else(|_| panic!("no hint within 5s: {what}"))
                .expect("the probe's queue stays open");
            assert!(
                matches!(
                    farhelm_proto::io::parse_control(&frame),
                    Ok(ControlMsg::SessionsChanged)
                ),
                "only hints are sent on the probe's link: {what}"
            );
        }

        /// Fail with `what` if a hint arrives within `window`.
        ///
        /// An absence is only evidence when the window outlasts the sender's
        /// coalescing gap ([`SESSIONS_CHANGED_MIN_GAP`]), so a mark made just
        /// before the call would have been sent inside it; callers pass
        /// several gaps' worth.
        pub(crate) async fn expect_no_hint(&mut self, window: Duration, what: &str) {
            assert!(
                window > SESSIONS_CHANGED_MIN_GAP * 2,
                "a window shorter than the coalescing gap proves nothing"
            );
            if let Ok(frame) = tokio::time::timeout(window, self.receiver.recv()).await {
                panic!("unexpected hint ({frame:?}): {what}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Register a helm link on `sup` with a writer queue of
    /// `capacity`, returning the queue's receiving end.
    async fn link(sup: &Arc<Supervisor>, capacity: usize) -> tokio::sync::mpsc::Receiver<Frame> {
        let (notify, receiver) = tokio::sync::mpsc::channel(capacity);
        let (shutdown, _) = tokio::sync::watch::channel(false);
        let _ = sup.register_helm_link(notify, shutdown).await;
        receiver
    }

    fn is_hint(frame: &Frame) -> bool {
        matches!(
            farhelm_proto::io::parse_control(frame),
            Ok(ControlMsg::SessionsChanged)
        )
    }

    /// Let every ready task run: the sender wakes on a mark, sends, and
    /// parks on its gap sleep, all without the paused clock moving.
    async fn settle() {
        for _ in 0..20 {
            tokio::task::yield_now().await;
        }
    }

    /// Spec: a mark reaches every registered helm connection as
    /// one `SessionsChanged` at once; marks made during the following
    /// [`SESSIONS_CHANGED_MIN_GAP`] send nothing until the gap ends, then go
    /// out as exactly one hint.
    ///
    /// Why: each hint costs the helm a whole listing round trip, so a burst
    /// must collapse; but the trailing edge must survive, or a change made
    /// just after a hint would wait for the backstop poll. The marks are
    /// spread across the gap with the sender given the chance to run
    /// between them, because marks made back to back would collapse in the
    /// `Notify` permit alone and say nothing about the gap.
    #[farhelm_testtrace::test]
    async fn marks_coalesce_into_one_hint_per_gap_and_keep_the_trailing_edge() {
        let state = farhelm_teststate::tempdir().expect("state dir");
        let sup = Supervisor::new(state.path()).await.expect("supervisor");
        let mut first = link(&sup, 8).await;
        let mut second = link(&sup, 8).await;
        let _sender = start_hint_sender(&sup);
        // Paused only now: building the supervisor does real I/O that a
        // paused clock's auto-advance has no business racing.
        tokio::time::pause();

        sup.hint_sessions_changed();
        settle().await;
        for receiver in [&mut first, &mut second] {
            let frame = receiver
                .try_recv()
                .expect("the first mark is hinted at once");
            assert!(is_hint(&frame), "a mark sends SessionsChanged");
        }

        // Five marks spread over the gap, the last one just before it ends.
        let step = SESSIONS_CHANGED_MIN_GAP / 5;
        for _ in 0..5 {
            sup.hint_sessions_changed();
            settle().await;
            assert!(
                first.try_recv().is_err(),
                "no hint may go out inside the gap"
            );
            tokio::time::advance(step - Duration::from_millis(1)).await;
            settle().await;
        }
        assert!(first.try_recv().is_err(), "still inside the gap");

        tokio::time::advance(Duration::from_millis(10)).await;
        settle().await;
        let trailing = first
            .try_recv()
            .expect("the marks are hinted once the gap ends");
        assert!(is_hint(&trailing));
        // Nothing further is owed: the burst collapsed into that one hint,
        // and no mark has been made since.
        tokio::time::advance(SESSIONS_CHANGED_MIN_GAP * 3).await;
        settle().await;
        assert!(
            first.try_recv().is_err(),
            "five marks inside one gap must produce one hint, not five"
        );
    }

    /// A filler frame that is not a hint, to occupy a writer queue's slots.
    fn filler() -> Frame {
        Frame::control(&ControlMsg::Detach { channel: 1 })
    }

    /// Spec: a hint that finds a connection's writer queue full is never
    /// waited for and never dropped: the other connections get theirs at
    /// once, and the full one is owed it and gets it at the next retry,
    /// [`OWED_HINT_RETRY`] later, once its queue has room. A retry wake sends
    /// nothing to a connection that owes nothing, and a delivered hint is
    /// owed no more.
    ///
    /// Why: marks come from paths holding supervisor state, so a hint must
    /// not wait on one slow helm; but the helm relies on hints to hear about
    /// a change promptly, so a dropped hint would leave its window stale until
    /// its backstop poll.
    #[farhelm_testtrace::test]
    async fn a_full_writer_queue_owes_the_hint_and_gets_it_at_the_retry() {
        let state = farhelm_teststate::tempdir().expect("state dir");
        let sup = Supervisor::new(state.path()).await.expect("supervisor");
        let (full_notify, mut full) = tokio::sync::mpsc::channel(1);
        full_notify.try_send(filler()).expect("fill the one slot");
        let (shutdown, _) = tokio::sync::watch::channel(false);
        let full_link = sup.register_helm_link(full_notify, shutdown).await;
        let mut healthy = link(&sup, 8).await;
        let _sender = start_hint_sender(&sup);
        // Paused only now, as in the coalescing test above.
        tokio::time::pause();

        sup.hint_sessions_changed();
        settle().await;
        let frame = healthy
            .try_recv()
            .expect("the healthy connection gets its hint at once");
        assert!(is_hint(&frame));
        let queued = full.try_recv().expect("the filler frame is still there");
        assert!(
            !is_hint(&queued),
            "the full queue kept only what it already held"
        );
        assert!(
            full.try_recv().is_err(),
            "the hint did not fit, so nothing else is queued"
        );
        assert!(
            full_link.hint_owed.lock().unwrap().is_some(),
            "the full connection owes it"
        );

        // Room now, but the retry is the sender's own timer, not the queue
        // draining: nothing arrives until the retry period has passed.
        tokio::time::advance(OWED_HINT_RETRY - Duration::from_millis(10)).await;
        settle().await;
        assert!(full.try_recv().is_err(), "not before the retry period");
        tokio::time::advance(Duration::from_millis(20)).await;
        settle().await;
        let owed = full
            .try_recv()
            .expect("the owed hint is delivered at the retry");
        assert!(is_hint(&owed));
        assert!(
            full_link.hint_owed.lock().unwrap().is_none(),
            "and is owed no more"
        );
        assert!(
            healthy.try_recv().is_err(),
            "a retry wake sends nothing to a connection that owed nothing"
        );

        tokio::time::advance(OWED_HINT_RETRY * 3).await;
        settle().await;
        assert!(full.try_recv().is_err(), "a delivered hint is owed no more");
    }

    /// Spec: while a connection owes a hint it gets at most one attempt per
    /// [`OWED_HINT_RETRY`], however often changes are marked and however
    /// often its queue empties in between; the attempt at the end of the
    /// period carries all of them, and once delivered the connection is back
    /// to the ordinary pace, a fresh mark hinted at once.
    ///
    /// Why: the maintainer's requirement that a queue teetering at its limit
    /// must not turn into a spam of refreshes (each hint costs the helm a
    /// whole session-list round trip, on a link already behind), while the
    /// last change still arrives.
    #[farhelm_testtrace::test]
    async fn an_owing_connection_is_tried_once_per_retry_period_whatever_is_marked() {
        let state = farhelm_teststate::tempdir().expect("state dir");
        let sup = Supervisor::new(state.path()).await.expect("supervisor");
        let (notify, mut queue) = tokio::sync::mpsc::channel(1);
        let filler_sender = notify.clone();
        let (shutdown, _) = tokio::sync::watch::channel(false);
        let owing = sup.register_helm_link(notify, shutdown).await;
        let _sender = start_hint_sender(&sup);
        tokio::time::pause();

        // The premise: the queue is full when the first mark is tried, so
        // the connection starts owing.
        filler_sender.try_send(filler()).expect("fill the one slot");
        sup.hint_sessions_changed();
        settle().await;
        assert!(
            owing.hint_owed.lock().unwrap().is_some(),
            "the premise: it owes"
        );
        assert!(
            !is_hint(&queue.try_recv().expect("the filler")),
            "only the filler was queued"
        );

        // Marks every 200 ms (the ordinary hint gap) for most of the retry
        // period, with the queue empty each time: a healthy connection would
        // have been hinted on every one of them.
        let step = SESSIONS_CHANGED_MIN_GAP;
        let mut elapsed = Duration::ZERO;
        while elapsed + step < OWED_HINT_RETRY {
            tokio::time::advance(step).await;
            elapsed += step;
            sup.hint_sessions_changed();
            settle().await;
            assert!(
                queue.try_recv().is_err(),
                "no attempt inside the retry period, {elapsed:?} in"
            );
        }

        tokio::time::advance(OWED_HINT_RETRY - elapsed).await;
        settle().await;
        assert!(is_hint(&queue.try_recv().expect("one hint at the retry")));
        tokio::time::advance(SESSIONS_CHANGED_MIN_GAP * 3).await;
        settle().await;
        assert!(
            queue.try_recv().is_err(),
            "and only one, for all those marks"
        );

        sup.hint_sessions_changed();
        settle().await;
        assert!(
            is_hint(&queue.try_recv().expect("back to the ordinary pace")),
            "a fresh mark on a connection that owes nothing is hinted at once"
        );
    }

    /// Spec: a connection whose queue is closed owes nothing: a mark that
    /// finds it closed leaves its flag clear, so the sender never wakes to
    /// retry it.
    ///
    /// Why: a closed queue is a connection on its way out; treating it like a
    /// full one would have the sender wake every retry period for it until it
    /// is unregistered.
    #[farhelm_testtrace::test]
    async fn a_closed_connection_owes_nothing() {
        let state = farhelm_teststate::tempdir().expect("state dir");
        let sup = Supervisor::new(state.path()).await.expect("supervisor");
        let (notify, receiver) = tokio::sync::mpsc::channel(1);
        let (shutdown, _) = tokio::sync::watch::channel(false);
        let closing = sup.register_helm_link(notify, shutdown).await;
        drop(receiver);
        let _sender = start_hint_sender(&sup);
        tokio::time::pause();

        sup.hint_sessions_changed();
        settle().await;
        assert!(
            closing.hint_owed.lock().unwrap().is_none(),
            "a closed queue owes nothing"
        );
    }

    /// Spec: when a mark arrives while the sender sleeps its gap and the
    /// gap ends after an owing connection's retry fell due, one pass serves
    /// both: the healthy connection gets its hint then, and the owing one
    /// gets exactly one, not one for the retry and another for the mark.
    ///
    /// Why: the sender's wait prefers a mark over the retry timer when both
    /// are ready (`biased`); taking the timer first would delay the healthy
    /// connection by a gap because of the slow one, and send the slow one a
    /// redundant hint, a refresh it cannot afford while it is behind.
    #[farhelm_testtrace::test]
    async fn a_mark_and_a_due_retry_ready_together_are_one_pass() {
        let state = farhelm_teststate::tempdir().expect("state dir");
        let sup = Supervisor::new(state.path()).await.expect("supervisor");
        let (owing_notify, mut owing_queue) = tokio::sync::mpsc::channel(1);
        owing_notify.try_send(filler()).expect("fill the one slot");
        let (shutdown, _) = tokio::sync::watch::channel(false);
        let owing = sup.register_helm_link(owing_notify, shutdown).await;
        let mut healthy = link(&sup, 8).await;
        let _sender = start_hint_sender(&sup);
        tokio::time::pause();

        sup.hint_sessions_changed();
        settle().await;
        assert!(is_hint(
            &healthy
                .try_recv()
                .expect("the healthy connection's first hint")
        ));
        assert!(
            owing.hint_owed.lock().unwrap().is_some(),
            "the premise: the other owes"
        );
        assert!(
            !is_hint(&owing_queue.try_recv().expect("the filler")),
            "now it has room"
        );

        // A mark just before the retry falls due: the owing connection is
        // not yet due, the healthy one is hinted, and the sender starts its
        // gap, which will end after the retry deadline.
        tokio::time::advance(OWED_HINT_RETRY - Duration::from_millis(100)).await;
        settle().await;
        sup.hint_sessions_changed();
        settle().await;
        assert!(is_hint(
            &healthy
                .try_recv()
                .expect("the healthy connection's second hint")
        ));
        assert!(owing_queue.try_recv().is_err(), "not yet due");

        // Inside that gap the retry falls due, and another mark arrives.
        tokio::time::advance(Duration::from_millis(150)).await;
        settle().await;
        sup.hint_sessions_changed();
        settle().await;
        tokio::time::advance(SESSIONS_CHANGED_MIN_GAP).await;
        settle().await;
        assert!(
            is_hint(
                &healthy
                    .try_recv()
                    .expect("the mark's pass reaches the healthy connection")
            ),
            "the healthy connection is not held back by the slow one"
        );
        assert!(is_hint(
            &owing_queue.try_recv().expect("the owing connection's hint")
        ));

        tokio::time::advance(OWED_HINT_RETRY * 2).await;
        settle().await;
        assert!(
            owing_queue.try_recv().is_err(),
            "exactly one hint for the owing connection"
        );
        assert!(
            healthy.try_recv().is_err(),
            "and nothing more for the healthy one"
        );
    }
}
