//! Durable conversation identities and report reconciliation.
//!
//! Only accepted reports introduce new identities. Historical stored identities
//! remain usable regardless of their source; this module never selects a vendor
//! record by searching a directory. Each refresh takes the session's report claim
//! before reading its row and verifying any exact file its integration requires.
//! The ticker and reply paths both refresh, so readiness converges even when no
//! helm polls the supervisor.

use super::core::{SessionEntry, Supervisor};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::warn;

/// The durable report write exposed to fault-injection tests.
/// A failed write must leave the in-memory Resume promise unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureWrite {
    /// The report handler's durable identity transaction.
    Report,
}

/// Inject a report-write failure without replacing the store itself.
/// Production installs none; tests use this to prove that memory never
/// advertises an identity whose transaction failed.
pub type CaptureStoreFault = Arc<dyn Fn(CaptureWrite, &str) -> anyhow::Result<()> + Send + Sync>;

/// Pause report admission at a named boundary in concurrency tests.
/// The boxed future lets a test supply an asynchronous barrier without changing
/// the production admission path or holding a synchronous lock across an await.
pub type CaptureGate =
    Arc<dyn Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + Sync>;

/// The durable identity mirrored for a session's offer surfaces.
///
/// Source is deliberately absent: historical identities and accepted reports
/// carry the same Resume promise. Readiness and ownership requirements remain
/// the integration's decision, with the durable ownership version alongside the
/// identity so memory and row-derived offers use the same evidence.
#[derive(Debug, Clone)]
pub(crate) enum CaptureState {
    /// No stored conversation is available; use the uncaptured fallback.
    Unclaimed,
    /// A committed identity, loaded from a row or installed after a report write.
    /// Another accepted report can replace it when the agent starts a new
    /// conversation. No speculative identity is ever published here.
    Reported {
        conversation: String,
        ownership_version: i64,
    },
}

impl CaptureState {
    /// The committed identity used to construct the session's restart offer.
    pub(crate) fn committed_conversation(&self) -> Option<&str> {
        match self {
            Self::Unclaimed => None,
            Self::Reported { conversation, .. } => Some(conversation),
        }
    }

    /// Ownership proof travels with the identity, including unknown versions.
    /// Callers must not normalize it: an integration can refuse an unfamiliar
    /// version while preserving the underlying data for a later build.
    pub(crate) fn committed_ownership_version(&self) -> Option<i64> {
        match self {
            Self::Unclaimed => None,
            Self::Reported {
                ownership_version, ..
            } => Some(*ownership_version),
        }
    }

    /// Publish a committed report, including replacements after `/clear` or `/new`.
    /// An empty observation cannot erase a stored identity; explicit withdrawal
    /// is represented by the integration's committed, non-ready locator.
    pub(crate) fn advance(&mut self, next: CaptureState) -> bool {
        if matches!(next, Self::Reported { .. }) {
            *self = next;
            true
        } else {
            false
        }
    }
}

/// Give injected reporters time to respond after the first delivered input.
/// This diagnostic budget is independent of identity admission: expiry neither
/// changes the restart offer nor prevents a later report from being accepted.
const REPORT_WARNING_AFTER: Duration = Duration::from_secs(65);

impl Supervisor {
    /// Refresh report-backed readiness before replies and on the periodic ticker.
    /// Each session's capture claim serializes its row and mirror update with
    /// report admission. No host-wide pass lock or scan is needed.
    pub(crate) async fn capture_now(&self) {
        let entries: Vec<Arc<SessionEntry>> =
            self.sessions.lock().await.values().cloned().collect();
        refresh_report_only_captures(self, &entries).await;
        report_liveness_tripwire(&entries, REPORT_WARNING_AFTER, Instant::now());
    }
}

/// Start the launch's diagnostic clock on confirmed input to its agent pane.
///
/// A launch can sit idle indefinitely before Codex has a prompt to report, so
/// launch time is not a useful anchor. Empty frames do not count; a partly
/// successful send does, since confirmed bytes reached the pane. Terminal replies
/// can also start this clock, which may cause an early diagnostic but never an
/// identity change. Nothing is persisted or spawned from the input path.
pub(crate) fn note_first_input(entry: &SessionEntry) {
    entry
        .run
        .first_input
        .lock()
        .expect("first-input mutex poisoned")
        .get_or_insert_with(Instant::now);
}

/// Publish an identity and immediately hint any changed restart offer.
/// A refresh may be cancelled after publishing one session, so delaying the hint
/// until the whole pass finishes would lose that session's update.
pub(crate) fn advance_capture(
    sup: &Supervisor,
    entry: &SessionEntry,
    state: &mut CaptureState,
    next: CaptureState,
) -> bool {
    let offer = |state: &CaptureState| {
        entry.snapshot.restart_offer(
            state.committed_conversation(),
            state.committed_ownership_version().unwrap_or(0),
        )
    };
    let before = offer(state);
    let advanced = state.advance(next);
    if offer(state) != before {
        sup.hint_sessions_changed();
    }
    advanced
}

/// Reconcile each reported row under the same claim used for admission.
/// Startup reports can precede entry publication, and exact-file verification
/// can withdraw readiness independently. Reloading only after taking the claim
/// prevents an older observation from overwriting a newer accepted report.
/// Historical non-hook identities are left intact without re-verification.
async fn refresh_report_only_captures(sup: &Supervisor, entries: &[Arc<SessionEntry>]) {
    for entry in entries {
        // Every integrated kind, asked of the integration seam rather than
        // listed here: a hand-written list silently skipped any kind added
        // later, leaving its reported identity unmirrored.
        if crate::agent_kind::integration_for(entry.snapshot.kind).is_none() {
            continue;
        }
        // The shared capture claim, unbounded here: this is a background
        // pass with no reporter waiting on it, so patience is correct and
        // a timeout would only trade convergence for a retry next pass.
        let _claim = sup.capture_locks.claim(&entry.info.id).await;
        let before = entry
            .run
            .capture
            .lock()
            .expect("capture mutex poisoned")
            .committed_conversation()
            .map(str::to_string);
        let mut row = match sup.store.session(&entry.info.id).await {
            Ok(Some(row)) => row,
            Ok(None) => continue,
            Err(error) => {
                warn!(session = %entry.info.id, %error, "could not refresh reported conversation");
                continue;
            }
        };
        if row.generation != entry.generation
            || row.agent_kind() != entry.snapshot.kind
            || row.conversation_source.as_deref() != Some("hook")
        {
            continue;
        }
        // The `_claimed` variant: this loop already holds this session's
        // capture claim (above), and the per-key mutex is not reentrant —
        // calling the claiming wrapper here parks the pass against itself.
        match sup.refresh_reported_capture_claimed(&mut row).await {
            Ok(true) => {}
            Ok(false) => continue,
            Err(error) => {
                warn!(session = %entry.info.id, %error, "could not refresh the exact reported capture");
                continue;
            }
        }
        let kind = row.agent_kind();
        let Some(conversation) = row.captured_conversation else {
            continue;
        };
        if !crate::agent_kind::accepts_reported_conversation(kind, &conversation) {
            continue;
        }
        let ownership_version = row.capture_ownership_version;
        let mut state = entry.run.capture.lock().expect("capture mutex poisoned");
        if state.committed_conversation() == before.as_deref() {
            advance_capture(
                sup,
                entry,
                &mut state,
                CaptureState::Reported {
                    conversation,
                    ownership_version,
                },
            );
        }
    }
}

/// Warn once when an injected hook has held no identity past its input budget.
///
/// The capture mutex makes the identity check and latch atomic against both
/// another evaluation and report publication. A durable report whose mirror is
/// not published yet can still produce one spurious line; this diagnostic does
/// not serialize admission merely to make the log tidier. Carried Resume
/// identities suppress it even when the new launch's hook stays silent.
/// The count exposes actual emissions to tests without a tracing subscriber.
fn report_liveness_tripwire(
    entries: &[Arc<SessionEntry>],
    timeout: Duration,
    now: Instant,
) -> usize {
    let ordering = std::sync::atomic::Ordering::Relaxed;
    let mut warned = 0;
    for entry in entries {
        if !entry.run.hooked.load(ordering) {
            continue;
        }
        let Some(at) = *entry
            .run
            .first_input
            .lock()
            .expect("first-input mutex poisoned")
        else {
            continue;
        };
        if now.saturating_duration_since(at) < timeout {
            continue;
        }
        let warn_now = {
            let state = entry.run.capture.lock().expect("capture mutex poisoned");
            let first =
                state.committed_conversation().is_none() && !entry.run.hook_warned.load(ordering);
            if first {
                entry.run.hook_warned.store(true, ordering);
            }
            first
        };
        if warn_now {
            warned += 1;
            warn!(session = %entry.info.id,
                "this session was launched with a conversation hook but holds no conversation identity");
        }
    }
    warned
}

#[cfg(test)]
mod tests {
    use super::super::core::tests::{StateDir, entry_with};
    use super::*;
    use crate::agent_kind::IntegrationSnapshot;
    use farhelm_proto::AgentKind;

    /// A later report replaces the current identity, but an empty observation
    /// cannot withdraw it. Otherwise an overlapping refresh could erase a report.
    #[test]
    fn only_another_identity_can_replace_a_stored_identity() {
        let mut state = CaptureState::Reported {
            conversation: "first".into(),
            ownership_version: 1,
        };
        assert!(state.advance(CaptureState::Reported {
            conversation: "second".into(),
            ownership_version: 2,
        }));
        assert_eq!(state.committed_conversation(), Some("second"));
        assert!(!state.advance(CaptureState::Unclaimed));
        assert_eq!(state.committed_conversation(), Some("second"));
    }

    /// Build only the launch state the warning reads, without starting an agent.
    /// The kind is varied to ensure this diagnostic follows injection rather
    /// than a hard-coded subset of integrations.
    fn entry(
        kind: AgentKind,
        hooked: bool,
        at: Option<Instant>,
        identity: bool,
    ) -> Arc<SessionEntry> {
        let mut entry = entry_with(None, crate::store::LastOutcome::Running);
        entry.snapshot = IntegrationSnapshot {
            kind,
            resume_template: None,
        };
        *entry.run.first_input.lock().unwrap() = at;
        entry
            .run
            .hooked
            .store(hooked, std::sync::atomic::Ordering::Relaxed);
        if identity {
            *entry.run.capture.lock().unwrap() = CaptureState::Reported {
                conversation: "stored-conversation".to_string(),
                ownership_version: 0,
            };
        }
        Arc::new(entry)
    }

    /// Every injected integration warns once after confirmed input, including
    /// Claude and Codex. Advancing the supplied clock proves both the exact
    /// boundary and the once-per-launch latch without scheduling or sleeps.
    #[test]
    fn hooked_launches_warn_once_after_input() {
        let at = Instant::now();
        for &kind in AgentKind::ALL {
            let entry = entry(kind, true, Some(at), false);
            let entries = [entry];
            assert_eq!(
                report_liveness_tripwire(
                    &entries,
                    REPORT_WARNING_AFTER,
                    at + REPORT_WARNING_AFTER - Duration::from_nanos(1)
                ),
                0,
                "{kind:?}"
            );
            assert_eq!(
                report_liveness_tripwire(&entries, REPORT_WARNING_AFTER, at + REPORT_WARNING_AFTER),
                1,
                "{kind:?}"
            );
            assert_eq!(
                report_liveness_tripwire(
                    &entries,
                    REPORT_WARNING_AFTER,
                    at + REPORT_WARNING_AFTER * 2
                ),
                0,
                "{kind:?}"
            );
        }
    }

    /// An accepted report before expiry suppresses the warning, just as an
    /// identity carried across Resume does. A launch that never received a
    /// hook or never took input has no diagnostic deadline at all.
    #[test]
    fn identities_and_unstarted_deadlines_stay_silent() {
        let at = Instant::now();
        for &kind in AgentKind::ALL {
            let reported = entry(kind, true, Some(at), false);
            assert!(
                reported
                    .run
                    .capture
                    .lock()
                    .unwrap()
                    .advance(CaptureState::Reported {
                        conversation: "accepted-report".to_string(),
                        ownership_version: 1,
                    })
            );
            let entries = [
                reported,
                entry(kind, true, Some(at), true),
                entry(kind, false, Some(at), false),
                entry(kind, true, None, false),
            ];
            assert_eq!(
                report_liveness_tripwire(&entries, REPORT_WARNING_AFTER, at + REPORT_WARNING_AFTER),
                0,
                "{kind:?}"
            );
        }
    }

    /// Spec: a capture-state advance that changes the session's restart
    /// offer marks a change hint before it returns, with no further await.
    ///
    /// Why: a capture pass publishes session by session and can be
    /// cancelled partway (a listing whose connection closes aborts the
    /// sweep it runs). Marking only when the whole pass finished would lose
    /// the hint for an offer already published, and the next pass would
    /// take that offer as its baseline and find nothing to hint.
    #[farhelm_testtrace::test]
    async fn an_advance_that_changes_the_restart_offer_hints_at_once() {
        let state = StateDir::new();
        let sup = Supervisor::new(state.path()).await.expect("supervisor");
        let mut hints = crate::service::hints::test_support::HintProbe::attach(&sup).await;
        let mut entry = entry_with(None, crate::store::LastOutcome::Running);
        entry.snapshot = IntegrationSnapshot {
            kind: AgentKind::Claude,
            resume_template: Some(vec![
                "claude".to_string(),
                "--resume".to_string(),
                "{conversation}".to_string(),
            ]),
        };
        let offer_before = super::super::status::session_restart_offer(&entry);

        let advanced = advance_capture(
            &sup,
            &entry,
            &mut entry.run.capture.lock().expect("capture mutex poisoned"),
            CaptureState::Reported {
                conversation: "0c3b1f6e-5a1d-4c55-9f7e-2d1f7b0a9e11".to_string(),
                ownership_version: 1,
            },
        );

        assert!(advanced, "fixture premise: the report advances the state");
        assert_ne!(
            super::super::status::session_restart_offer(&entry),
            offer_before,
            "fixture premise: the report changes the restart offer"
        );
        hints.expect_hint("the changed restart offer").await;
    }
}
