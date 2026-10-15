//! Configuration saves fence later requests from this GUI, without serializing them.
//!
//! A ticket means queued or in-flight work, not a dirty value awaiting credential
//! recovery. A dependent request snapshots tickets once: newer saves cannot extend
//! its wait. Dropping a ticket releases readers even on cancellation or failure.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use futures_channel::oneshot;

/// Shared across remounted components; tests use independent instances.
#[derive(Clone, Default)]
pub(super) struct Saves(Arc<Mutex<State>>);

/// Sequence identities let queued batches settle independently; each active
/// ticket owns only the readers that captured it before it settled.
#[derive(Default)]
struct State {
    next: u64,
    pending: BTreeMap<u64, Vec<oneshot::Sender<()>>>,
}

/// Own one save's place in the order, including time queued behind a writer.
/// Settlement is deliberately outcome-independent: a lost reply is still an
/// unknown remote outcome, and cannot hold all future reads indefinitely.
pub(super) struct Ticket {
    saves: Saves,
    id: u64,
}

impl Saves {
    /// Register synchronously, before the caller spawns or polls the save.
    pub(super) fn register(&self) -> Ticket {
        let mut state = self.0.lock().expect("save order lock poisoned");
        state.next = state.next.checked_add(1).expect("save sequence exhausted");
        let id = state.next;
        state.pending.insert(id, Vec::new());
        Ticket {
            saves: self.clone(),
            id,
        }
    }

    /// Capture only preceding saves; dropped waiters are pruned on later reads.
    /// The receivers are independent, so no mutex remains held across an await.
    pub(super) fn preceding(&self) -> impl Future<Output = ()> + use<> {
        let waiting: Vec<_> = self
            .0
            .lock()
            .expect("save order lock poisoned")
            .pending
            .values_mut()
            .map(|waiters| {
                waiters.retain(|sender| !sender.is_canceled());
                let (sender, receiver) = oneshot::channel();
                waiters.push(sender);
                receiver
            })
            .collect();
        async move {
            for receiver in waiting {
                let _ = receiver.await;
            }
        }
    }
}

impl Drop for Ticket {
    fn drop(&mut self) {
        let waiters = self
            .saves
            .0
            .lock()
            .expect("save order lock poisoned")
            .pending
            .remove(&self.id)
            .unwrap_or_default();
        // Wake outside the mutex: the executor may poll a reader immediately.
        for sender in waiters {
            let _ = sender.send(());
        }
    }
}

/// Charge the ordering wait to the request's existing deadline on both renderers.
/// An expired waiter never dispatches HTTP; the remaining budget goes to reqwest,
/// whose timeout continues through response-body reads.
pub(super) async fn remaining(
    waiting: impl Future<Output = ()>,
    started: web_time::Instant,
    timeout: std::time::Duration,
) -> Result<std::time::Duration, super::SendError> {
    let budget = timeout.saturating_sub(started.elapsed());
    let mut waiting = std::pin::pin!(waiting);
    let mut deadline = std::pin::pin!(crate::reader::sleep_ms(budget.as_millis() as u64));
    std::future::poll_fn(|cx| {
        if started.elapsed() >= timeout || deadline.as_mut().poll(cx).is_ready() {
            return std::task::Poll::Ready(Err(super::SendError::Request(
                "request timed out waiting for earlier configuration saves".into(),
            )));
        }
        if waiting.as_mut().poll(cx).is_ready() {
            return std::task::Poll::Ready(Ok(timeout.saturating_sub(started.elapsed())));
        }
        std::task::Poll::Pending
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::task::{Context, Poll, Waker};

    /// Poll directly so pending is an observed scheduler state, not an elapsed
    /// observation window that might pass before the future actually starts.
    fn poll<F: Future>(future: std::pin::Pin<&mut F>) -> Poll<F::Output> {
        future.poll(&mut Context::from_waker(Waker::noop()))
    }

    /// An idle GUI must dispatch immediately; unrelated newer saves cannot
    /// retroactively turn an already-started request into a dependent one.
    #[farhelm_testtrace::test]
    fn idle_snapshot_does_not_wait_for_later_save() {
        let saves = Saves::default();
        let mut waiting = std::pin::pin!(saves.preceding());
        let _later = saves.register();
        assert!(poll(waiting.as_mut()).is_ready());
    }

    /// Wait for every earlier save, even if independent saves finish out of
    /// order. New traffic cannot starve the captured request.
    #[farhelm_testtrace::test]
    fn waits_for_all_earlier_saves_but_not_newer_ones() {
        let saves = Saves::default();
        let first = saves.register();
        let second = saves.register();
        let mut waiting = std::pin::pin!(saves.preceding());
        assert!(poll(waiting.as_mut()).is_pending());
        let _later = saves.register();
        drop(second);
        assert!(poll(waiting.as_mut()).is_pending());
        drop(first);
        assert!(poll(waiting.as_mut()).is_ready());
    }

    /// A cancelled unpolled save still owns its ticket; dropping it must free
    /// subsequent reads without depending on any task having run first.
    #[farhelm_testtrace::test]
    fn cancelling_before_first_poll_releases_waiter() {
        let saves = Saves::default();
        let ticket = saves.register();
        let save = async move {
            let _ticket = ticket;
            std::future::pending::<()>().await;
        };
        let mut waiting = std::pin::pin!(saves.preceding());
        assert!(poll(waiting.as_mut()).is_pending());
        drop(save);
        assert!(poll(waiting.as_mut()).is_ready());
    }

    /// Abandoning a read does not cancel the saves it observed or leave a
    /// permanent waiter; a later read can still observe their actual lifetime.
    #[farhelm_testtrace::test]
    fn cancelled_waiters_are_pruned_without_cancelling_saves() {
        let saves = Saves::default();
        let ticket = saves.register();
        drop(saves.preceding());
        let mut waiting = std::pin::pin!(saves.preceding());
        assert_eq!(saves.0.lock().unwrap().pending[&ticket.id].len(), 1);
        assert!(poll(waiting.as_mut()).is_pending());
        drop(ticket);
        assert!(poll(waiting.as_mut()).is_ready());
    }

    /// Waiting consumes the existing request budget rather than adding a fresh
    /// full timeout after the saves finish. An expired request never dispatches.
    #[farhelm_testtrace::test]
    async fn deadline_includes_time_since_funnel_entry() {
        use std::time::Duration;
        let saves = Saves::default();
        let _ticket = saves.register();
        let started = web_time::Instant::now() - Duration::from_secs(2);
        let result = remaining(saves.preceding(), started, Duration::from_secs(1)).await;
        assert!(
            matches!(result, Err(super::super::SendError::Request(text)) if text.contains("timed out"))
        );
        let budget = remaining(std::future::ready(()), started, Duration::from_secs(5))
            .await
            .unwrap();
        assert!(budget <= Duration::from_secs(3));
        // Bound against the measured elapsed time, so a stalled test thread
        // cannot turn a correct budget into a failure.
        assert!(budget >= Duration::from_secs(5).saturating_sub(started.elapsed()));
    }

    /// A read queued behind a save that never settles must give up when its
    /// own budget runs out. The timer wake is what bounds that wait, so the
    /// deadline here is still in the future when the wait begins.
    #[farhelm_testtrace::test]
    async fn a_hung_save_releases_waiters_at_their_deadline() {
        use std::time::Duration;
        let saves = Saves::default();
        let _hung = saves.register();
        let started = web_time::Instant::now();
        let result = remaining(saves.preceding(), started, Duration::from_millis(30)).await;
        assert!(
            matches!(result, Err(super::super::SendError::Request(text)) if text.contains("timed out waiting for earlier configuration saves"))
        );
        assert!(started.elapsed() >= Duration::from_millis(30));
    }
}
