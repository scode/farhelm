//! Change hints: telling connected helms "this host's sessions changed, poll
//! now" (`ControlMsg::SessionsChanged`, protocol 33).
//!
//! A hint carries nothing but its arrival. The helm answers one by listing
//! this supervisor's sessions through its ordinary `ListSessions` path, so
//! nothing here has to know what changed, only THAT something a user can
//! see did. The alternative, pushing rows or deltas, was rejected: much of a
//! listed row is derived as the reply is built (tabs, status, the restart
//! offer), so every such field would need its own detector and snapshot, and
//! a missed one would leave the helm wrong with nothing to correct it. A
//! missed hint costs one poll interval, because the helm keeps polling as
//! the backstop.
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
//! To every full-authority connection (every registered
//! [`super::agent_relay::HelmLink`]), whatever its hello's `role` says: the
//! role is diagnostic text, not an authorization input, and this supervisor
//! cannot tell a helm from any other full-authority client. A connection
//! that does not show sessions ignores the message. SPEC.md runs one helm at
//! a time, so in practice this is that helm.
//!
//! Each hint is `try_send` on the connection's ordinary writer queue, behind
//! whatever terminal output is already queued. A full queue DROPS the hint:
//! a hint must never block the supervisor, and a helm too far behind to take
//! one is served by its backstop poll.

use super::core::Supervisor;
use farhelm_proto::{ControlMsg, Frame, SESSIONS_CHANGED_MIN_GAP};
use std::sync::Arc;
use tokio::sync::Notify;

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
    /// Every path that records an observed exit or error (the ticker, a
    /// listing, a single-session read) mirrors through here, because
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
            changed
        };
        if changed {
            self.hint_sessions_changed();
        }
    }

    /// Send one `SessionsChanged` to every registered full-authority
    /// connection, dropping it on any whose writer queue is full.
    async fn send_sessions_changed(&self) {
        let frame = Frame::control(&ControlMsg::SessionsChanged);
        for link in self.helm_links.lock().await.iter() {
            // Dropped on a full or closed queue alike: the backstop poll
            // covers the first, and the second is a connection on its way
            // out.
            let _ = link.notify.try_send(frame.clone());
        }
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
/// Holds the supervisor only weakly between hints, so the task never keeps a
/// supervisor alive that everything else has let go of.
pub(crate) fn start_hint_sender(sup: &Arc<Supervisor>) -> HintSender {
    let hints = Arc::clone(&sup.change_hints);
    let sup = Arc::downgrade(sup);
    let task = tokio::spawn(async move {
        loop {
            hints.pending.notified().await;
            let Some(sup) = sup.upgrade() else {
                return;
            };
            sup.send_sessions_changed().await;
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

    /// A registered full-authority link plus a running sender: what `serve`
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Register a full-authority link on `sup` with a writer queue of
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

    /// Spec: a mark reaches every registered full-authority connection as
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

    /// Spec: a hint is dropped, never waited for, on a connection whose
    /// writer queue is full, and the other connections still get theirs.
    ///
    /// Why: marks come from paths holding supervisor state, and a hint that
    /// waited on one slow helm would stall them; the backstop poll covers
    /// the dropped hint.
    #[farhelm_testtrace::test]
    async fn a_full_writer_queue_drops_the_hint_without_blocking() {
        let state = farhelm_teststate::tempdir().expect("state dir");
        let sup = Supervisor::new(state.path()).await.expect("supervisor");
        let (full_notify, mut full) = tokio::sync::mpsc::channel(1);
        full_notify
            .try_send(Frame::control(&ControlMsg::Detach { channel: 1 }))
            .expect("fill the one slot");
        let (shutdown, _) = tokio::sync::watch::channel(false);
        let _ = sup.register_helm_link(full_notify, shutdown).await;
        let mut healthy = link(&sup, 8).await;
        let _sender = start_hint_sender(&sup);

        sup.hint_sessions_changed();
        let frame = tokio::time::timeout(Duration::from_secs(5), healthy.recv())
            .await
            .expect("the healthy connection gets its hint")
            .expect("the queue is open");
        assert!(is_hint(&frame));
        let queued = full.recv().await.expect("the filler frame is still there");
        assert!(
            !is_hint(&queued),
            "the full queue kept only what it already held"
        );
        assert!(full.try_recv().is_err(), "the hint was dropped, not queued");
    }
}
