//! End timed mutes on the helm, including when no window is open.
//!
//! The fleet feed already carries every write that can move the earliest
//! deadline. Subscribe before reading that deadline so a new mute cannot
//! land between the query and the wait.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::AppState;
use crate::manager::HostStatus;
use crate::store::{MuteObservation, SessionKey};

/// Tokio's clock is monotonic. Recheck wall time while a deadline is armed
/// so a clock correction or suspend cannot leave a timed mute on for hours.
const MAX_DEADLINE_WAIT: Duration = Duration::from_secs(60);

/// Endpoints and the timer use one wall-clock unit for persisted deadlines.
pub(crate) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|time| i64::try_from(time.as_millis()).ok())
        .unwrap_or(0)
}

/// Identity-less hosts carry their observations in memory. A durable
/// observation is read by the store under the mute transition's transaction.
pub(crate) fn live_observation(
    key: &SessionKey,
    status: Option<&HostStatus>,
) -> Option<MuteObservation> {
    status?
        .live_sessions
        .as_deref()?
        .iter()
        .find(|info| info.id == key.session_id)
        .map(MuteObservation::from)
}

/// Expire at the latest observed session state. The store rechecks each
/// deadline before it applies the seen stamp, so later choices win a race.
async fn expire(state: &AppState, now: i64) -> anyhow::Result<()> {
    for (key, deadline) in state.store.due_mutes(now).await? {
        let status = state.manager.status(key.host);
        let observed = live_observation(&key, status.as_ref());
        if state
            .store
            .unmute_session(&key, Some(deadline), observed)
            .await?
        {
            state.manager.events().bump();
        }
    }
    Ok(())
}

/// Ties the timer's lifetime to the helm's serve loop, including early returns.
pub(crate) struct ExpiryTask(tokio::task::JoinHandle<()>);

impl Drop for ExpiryTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}

impl ExpiryTask {
    /// A mute whose deadline passed while the helm was down ends on startup.
    pub(crate) fn start(state: &Arc<AppState>) -> Self {
        let mut revisions = state.manager.events().subscribe();
        let state = Arc::downgrade(state);
        Self(tokio::spawn(async move {
            loop {
                revisions.borrow_and_update();
                let Some(state) = state.upgrade() else { return };
                let scan = async {
                    expire(&state, now_ms()).await?;
                    state.store.next_mute_deadline().await
                }
                .await;
                drop(state);
                let delay = match scan {
                    Ok(Some(deadline)) => Some(
                        Duration::from_millis(deadline.saturating_sub(now_ms()).max(0) as u64)
                            .min(MAX_DEADLINE_WAIT),
                    ),
                    Ok(None) => None,
                    Err(error) => {
                        tracing::warn!(%error, "session mute expiry scan failed");
                        Some(Duration::from_secs(5))
                    }
                };
                tokio::select! {
                    result = revisions.changed() => if result.is_err() { return },
                    () = wait_until_due(delay) => {}
                }
            }
        }))
    }
}

/// An indefinite mute never schedules a timer. Only a fleet revision can
/// wake that scan until a timed mute is stored.
async fn wait_until_due(delay: Option<Duration>) {
    match delay {
        Some(delay) => tokio::time::sleep(delay).await,
        None => std::future::pending::<()>().await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rest_harness;

    /// A deadline that passed while the helm was down is settled on
    /// startup, even with no client connected and no new host activity.
    #[farhelm_testtrace::test]
    async fn startup_expires_a_due_mute_without_a_client() {
        let mut info = rest_harness::session("quiet", 1_700_000_000);
        info.last_activity_at = 1_700_000_090;
        let harness = rest_harness::helm_listing(vec![info.clone()]).await;
        let key = SessionKey::new(rest_harness::local_id(&harness.store).await, "quiet");
        let keys = std::slice::from_ref(&key);
        let deadline = now_ms() - 1;
        assert_eq!(
            harness
                .store
                .mute_session(&key, Some(deadline), None)
                .await
                .unwrap(),
            Some(true)
        );
        assert_eq!(
            harness.store.next_mute_deadline().await.unwrap(),
            Some(deadline)
        );
        let mut events = harness.manager.events().subscribe();
        let _task = ExpiryTask::start(&harness.state);
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                events.changed().await.unwrap();
                if harness.store.session_mutes(keys).await.unwrap()[&key]
                    .active()
                    .is_none()
                {
                    break;
                }
            }
        })
        .await
        .expect("the timer should publish the expired mute without an HTTP request");
        assert_eq!(
            harness.store.seen_activity(keys).await.unwrap()[&key],
            info.effective_activity()
        );
        assert_eq!(harness.store.next_mute_deadline().await.unwrap(), None);
    }
}
