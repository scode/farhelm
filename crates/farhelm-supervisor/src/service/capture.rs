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
    /// Apply the reports hooks have dropped, then refresh report-backed
    /// readiness, before replies and on the periodic ticker. Each session's
    /// capture claim serializes its row and mirror update with report
    /// admission; dropped reports are applied first so a refresh, and any
    /// Restart that reads capture state after this pass, sees them — unless
    /// another drain is already running, in which case that drain applies
    /// them and this pass does not wait for it.
    pub(crate) async fn capture_now(&self) {
        self.capture_pass(false).await;
    }

    /// [`Supervisor::capture_now`], with the choice of whether the report
    /// drain waits for one already running (see `apply_report_files`).
    pub(crate) async fn capture_pass(&self, wait_for_drain: bool) {
        let entries: Vec<Arc<SessionEntry>> =
            self.sessions.lock().await.values().cloned().collect();
        self.apply_report_files(&entries, wait_for_drain).await;
        refresh_report_only_captures(self, &entries).await;
        let silent = report_liveness_tripwire(&entries, REPORT_WARNING_AFTER, Instant::now());
        for entry in silent {
            self.notify_hook_silent(&entry).await;
        }
        self.resolve_resumable_notifications(&entries).await;
    }

    /// Turn one tripwire firing into the session's notification (SPEC.md,
    /// Status). The store records it only while that launch's row still holds
    /// no captured conversation, checked in the same statement as the insert:
    /// a report committed between this pass's mirror refresh and the tripwire
    /// is not in the capture state yet, and a notification, unlike the log
    /// line, must be true when recorded (the plan's M10).
    ///
    /// A launch already told that its OMP reporter does not match is not told
    /// again that its hook never reported: it is the same cause, and that
    /// notification already says what the user loses.
    ///
    /// An agent whose report is only due after its first reply (Pi, OMP;
    /// `RestartReadiness::due_by_first_prompt`) is not told at all: a first
    /// turn can outlast any wait, so the silence proves nothing. The log line
    /// the tripwire writes stays, as the diagnostic it always was.
    async fn notify_hook_silent(&self, entry: &SessionEntry) {
        if !entry
            .snapshot
            .kind
            .restart_readiness()
            .is_some_and(farhelm_proto::RestartReadiness::due_by_first_prompt)
        {
            return;
        }
        let id = entry.info.id.as_str();
        let mismatch = super::notifications::NotificationKind::ReporterMismatch.as_str();
        match self
            .store
            .has_session_notification(id, entry.generation, mismatch)
            .await
        {
            Ok(false) => {}
            Ok(true) => return,
            Err(error) => {
                warn!(session = %id, error = %format!("{error:#}"),
                    "could not check a silent hook's other notifications");
                return;
            }
        }
        self.notify_session(
            entry.info.id.as_str(),
            entry.generation,
            super::notifications::NotificationKind::HookSilent,
            super::notifications::HOOK_SILENT_TEXT,
        )
        .await;
    }
}

/// Start the launch's diagnostic clock on the first submitted line delivered
/// to its agent pane.
///
/// A launch can sit idle indefinitely before Codex has a prompt to report, so
/// launch time is not a useful anchor, and neither is any input at all: the
/// terminal answers the agent TUI's own queries (device attributes, cursor
/// position, colour queries, focus reports) without the user typing anything,
/// and a clock started by those used to fire for an agent nobody had prompted.
/// The caller therefore calls this only for delivered input containing a
/// carriage return, which none of those replies carries (checked against the
/// vendored xterm.js: CSI and DCS replies, and OSC answers terminated by ST).
/// That firing is now a user-facing notification, so it must be true (SPEC.md,
/// Status). A partly successful send counts, since confirmed bytes reached the
/// pane. Nothing is persisted or spawned from the input path.
pub(crate) fn note_first_input(entry: &SessionEntry) {
    entry
        .run
        .first_input
        .lock()
        .expect("first-input mutex poisoned")
        .get_or_insert_with(Instant::now);
}

/// Whether one input frame to the agent pane submits a line: whether it holds
/// an Enter that is not part of something else.
///
/// The silent-hook clock starts on the first such frame ([`note_first_input`]),
/// because an agent has nothing to report before the user submits something,
/// and its firing is a user-facing notification that must be true (SPEC.md,
/// Status). A carriage return counts unless it is:
///
/// - preceded by ESC: Farhelm's own Shift+Enter (and Alt+Enter) sends `ESC CR`
///   precisely so the agent inserts a newline instead of submitting
///   (`shift-enter-key.js`);
/// - inside a bracketed paste (`ESC [200~` .. `ESC [201~`) in the same frame:
///   xterm.js turns a pasted newline into a carriage return, and an agent with
///   bracketed paste enabled keeps the paste in its composer unsubmitted.
///
/// The terminal's automatic replies to the agent's own queries carry no
/// carriage return at all (checked against the vendored xterm.js). Residuals,
/// accepted: an Enter that answers a dialog (a trust prompt, a picker) counts,
/// and a paste large enough to be split across frames has its middle frames
/// judged without their markers.
pub(crate) fn submits_a_line(frame: &[u8]) -> bool {
    const PASTE_START: &[u8] = b"\x1b[200~";
    const PASTE_END: &[u8] = b"\x1b[201~";
    let mut in_paste = false;
    let mut index = 0;
    while index < frame.len() {
        let rest = &frame[index..];
        if rest.starts_with(PASTE_START) {
            in_paste = true;
            index += PASTE_START.len();
            continue;
        }
        if rest.starts_with(PASTE_END) {
            in_paste = false;
            index += PASTE_END.len();
            continue;
        }
        if frame[index] == b'\r' && !in_paste && (index == 0 || frame[index - 1] != 0x1b) {
            return true;
        }
        index += 1;
    }
    false
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
/// Returns the entries it warned for, which the capture pass turns into
/// notifications (this function holds a std mutex and records nothing
/// itself) and which expose actual emissions to tests without a tracing
/// subscriber.
fn report_liveness_tripwire(
    entries: &[Arc<SessionEntry>],
    timeout: Duration,
    now: Instant,
) -> Vec<Arc<SessionEntry>> {
    let ordering = std::sync::atomic::Ordering::Relaxed;
    let mut warned = Vec::new();
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
            warned.push(Arc::clone(entry));
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
                )
                .len(),
                0,
                "{kind:?}"
            );
            assert_eq!(
                report_liveness_tripwire(&entries, REPORT_WARNING_AFTER, at + REPORT_WARNING_AFTER)
                    .len(),
                1,
                "{kind:?}"
            );
            assert_eq!(
                report_liveness_tripwire(
                    &entries,
                    REPORT_WARNING_AFTER,
                    at + REPORT_WARNING_AFTER * 2
                )
                .len(),
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
                report_liveness_tripwire(&entries, REPORT_WARNING_AFTER, at + REPORT_WARNING_AFTER)
                    .len(),
                0,
                "{kind:?}"
            );
        }
    }

    /// Seed `id`'s stored row the way a hooked Claude launch leaves it,
    /// with or without a captured conversation, and publish a matching entry
    /// whose submitted-line clock started past the tripwire's budget.
    /// Its published snapshot omits a resume command to isolate the tripwire;
    /// tests of recovery must add one before claiming Resume is available.
    async fn silent_hook_session(sup: &Supervisor, id: &str, captured: Option<&str>) {
        silent_hook_session_of(sup, id, captured, AgentKind::Claude).await;
    }

    /// [`silent_hook_session`] for a launch integrated as `kind`, the part
    /// the silent-hook rule asks about.
    async fn silent_hook_session_of(
        sup: &Supervisor,
        id: &str,
        captured: Option<&str>,
        kind: AgentKind,
    ) {
        sup.store
            .insert_session(
                crate::store::StoredSession {
                    conversation_source: captured.map(|_| "hook".to_string()),
                    capture_ownership_version: 0,
                    omp_reporter_asset: None,
                    omp_launch_program: None,
                    id: id.to_string(),
                    parent: None,
                    title: id.to_string(),
                    created_at: 1_700_000_000,
                    last_activity_at: 1_700_000_000,
                    last_work_started_at: 0,
                    creation_seq: 0,
                    cwd: "/tmp".to_string(),
                    launch: farhelm_proto::SessionLaunch::Legacy {
                        invocation: "claude".to_string(),
                        agent_kind: AgentKind::Claude,
                        resume_template: Some(vec![
                            "claude".to_string(),
                            "--resume".to_string(),
                            "{conversation}".to_string(),
                        ]),
                    },
                    tmux_name: format!("fh-{id}"),
                    pane: String::new(),
                    outcome: crate::store::LastOutcome::Running,
                    canonical_cwd: None,
                    captured_conversation: captured.map(str::to_string),
                    generation: 0,
                    launch_scoped: false,
                },
                None,
            )
            .await
            .expect("seed the session row");
        let started = Instant::now()
            .checked_sub(REPORT_WARNING_AFTER + Duration::from_secs(1))
            .expect("a clock that has run for a minute");
        let mut published = entry_with(None, crate::store::LastOutcome::Running);
        published.info.id = id.to_string();
        published.snapshot = IntegrationSnapshot {
            kind,
            resume_template: None,
        };
        *published.run.first_input.lock().unwrap() = Some(started);
        published
            .run
            .hooked
            .store(true, std::sync::atomic::Ordering::Relaxed);
        sup.sessions
            .lock()
            .await
            .insert(id.to_string(), Arc::new(published));
    }

    /// Spec (SPEC.md, Status): a hooked launch that has not reported a
    /// minute after the first submitted line gets exactly one notification,
    /// carried on the session's replies; a launch whose identity is already
    /// in the store gets none.
    ///
    /// Why: this is the tripwire turned user-facing, and the two ways it
    /// could lie are both pinned here. Repeating it every pass would bury
    /// the session's list in copies; and the in-memory check alone can miss
    /// a report the store committed a moment ago (the plan's M10), which the
    /// store re-read is there to catch, so the second session's mirror is
    /// deliberately left without the identity its row holds.
    #[farhelm_testtrace::test]
    async fn a_silent_hook_notifies_once_unless_the_store_holds_an_identity() {
        let state = StateDir::new();
        let sup = Supervisor::new(state.path()).await.expect("supervisor");
        silent_hook_session(&sup, "silent", None).await;
        silent_hook_session(&sup, "reported", Some("stored-conversation")).await;

        sup.capture_pass(true).await;
        sup.capture_pass(true).await;

        let silent = sup
            .store
            .session_notifications("silent")
            .await
            .expect("list");
        assert_eq!(silent.len(), 1, "one notification per launch: {silent:?}");
        assert_eq!(
            silent[0].text,
            super::super::notifications::HOOK_SILENT_TEXT
        );
        let entry = sup
            .sessions
            .lock()
            .await
            .get("silent")
            .cloned()
            .expect("entry");
        let info = super::super::status::entry_info(
            &entry,
            &std::collections::HashMap::new(),
            &Default::default(),
            None,
        );
        assert_eq!(
            info.notifications, silent,
            "replies carry the recorded notification"
        );

        assert!(
            sup.store
                .session_notifications("reported")
                .await
                .expect("list")
                .is_empty(),
            "a stored identity suppresses the notification"
        );
    }

    /// A late accepted identity must resolve the existing silent-hook warning,
    /// including a warning read from storage by a later supervisor. The normal
    /// capture pass must do this; relying only on the report transition misses
    /// startup admission before the session entry exists.
    #[farhelm_testtrace::test]
    async fn a_late_report_resolves_silent_hook_history_across_supervisor_restart() {
        for restart in [false, true] {
            let state = StateDir::new();
            let mut sup = Supervisor::new(state.path()).await.unwrap();
            silent_hook_session(&sup, "late", None).await;
            sup.capture_pass(true).await;
            let before = sup.store.session_notifications("late").await.unwrap();
            assert_eq!(before.len(), 1);
            assert!(
                !before[0].resolved,
                "fixture must first record a real warning"
            );
            assert!(
                sup.store
                    .replace_reported_conversation_if_current("late", 0, None, "late-conversation")
                    .await
                    .unwrap()
            );
            if restart {
                // Admission may persist before startup publishes an entry.
                // The constructor must load the warning and reconcile it.
                drop(sup);
                sup = Supervisor::new(state.path()).await.unwrap();
            } else {
                // The tripwire fixture does not normally need a resume command;
                // this scenario must carry one to prove Resume really returns.
                let mut published = sup.sessions.lock().await.remove("late").unwrap();
                Arc::get_mut(&mut published)
                    .unwrap()
                    .snapshot
                    .resume_template = Some(vec![
                    "claude".into(),
                    "--resume".into(),
                    "{conversation}".into(),
                ]);
                sup.sessions.lock().await.insert("late".into(), published);
                sup.capture_pass(true).await;
            }
            let published = sup.sessions.lock().await.get("late").cloned().unwrap();
            assert_eq!(
                super::super::status::session_restart_offer(&published),
                farhelm_proto::RestartOffer::Resume,
                "the late identity must actually restore Resume, restart={restart}"
            );
            let resolved = sup.store.session_notifications("late").await.unwrap();
            assert!(resolved[0].resolved, "restart={restart}");
            assert_eq!(
                (resolved[0].seq, resolved[0].at, &resolved[0].text),
                (before[0].seq, before[0].at, &before[0].text)
            );
            assert_eq!(
                *published.session.notifications.lock().unwrap(),
                resolved,
                "the listing cell must publish the resolution, restart={restart}"
            );
        }
    }

    /// Spec (SPEC.md, Status): an agent that reports only after its first
    /// reply (Pi here; OMP alike) is never told its hook is silent, because a
    /// first turn can outlast any wait and the notification has to be true.
    #[farhelm_testtrace::test]
    async fn a_silent_hook_does_not_notify_for_an_agent_that_reports_after_its_reply() {
        let state = StateDir::new();
        let sup = Supervisor::new(state.path()).await.expect("supervisor");
        silent_hook_session_of(&sup, "pi-turn", None, AgentKind::Pi).await;

        sup.capture_pass(true).await;

        assert!(
            sup.store
                .session_notifications("pi-turn")
                .await
                .expect("list")
                .is_empty()
        );
    }

    /// Spec: the silent-hook notification is refused when the launch's
    /// stored row already holds a captured conversation even though the
    /// in-memory mirror does not yet (the store checks it in the same
    /// statement as the insert), and when the launch already has a
    /// reporter-mismatch notification (one cause, one notification).
    ///
    /// Why: the capture pass refreshes the mirror before the tripwire, so the
    /// end-to-end test above never reaches the stale-mirror case; calling the
    /// recorder directly with a stale mirror is what pins the durable guard.
    #[farhelm_testtrace::test]
    async fn a_silent_hook_is_not_recorded_over_a_stored_identity_or_a_reporter_mismatch() {
        let state = StateDir::new();
        let sup = Supervisor::new(state.path()).await.expect("supervisor");
        silent_hook_session(&sup, "stale-mirror", Some("stored-conversation")).await;
        let entry = sup
            .sessions
            .lock()
            .await
            .get("stale-mirror")
            .cloned()
            .expect("entry");
        assert!(
            entry
                .run
                .capture
                .lock()
                .unwrap()
                .committed_conversation()
                .is_none(),
            "premise: the mirror has not caught up with the stored identity"
        );
        sup.notify_hook_silent(&entry).await;
        assert!(
            sup.store
                .session_notifications("stale-mirror")
                .await
                .expect("list")
                .is_empty(),
            "a stored identity refuses the notification"
        );

        silent_hook_session(&sup, "mismatched", None).await;
        let entry = sup
            .sessions
            .lock()
            .await
            .get("mismatched")
            .cloned()
            .expect("entry");
        sup.notify_session(
            "mismatched",
            entry.generation,
            super::super::notifications::NotificationKind::ReporterMismatch,
            "mismatch",
        )
        .await;
        sup.notify_hook_silent(&entry).await;
        let texts: Vec<String> = sup
            .store
            .session_notifications("mismatched")
            .await
            .expect("list")
            .into_iter()
            .map(|notification| notification.text)
            .collect();
        assert_eq!(
            texts,
            vec!["mismatch".to_string()],
            "one cause, one notification"
        );
    }

    /// Spec: an Enter submits a line; Shift+Enter (`ESC CR`), a carriage
    /// return inside a bracketed paste, and input with no carriage return at
    /// all (the terminal's automatic replies) do not.
    ///
    /// Why: these are the inputs that start the silent-hook clock, whose
    /// firing tells the user Restart cannot resume. Counting a newline the
    /// user inserted while still composing their first prompt would tell them
    /// that a minute before the agent had anything to report.
    #[test]
    fn only_a_real_enter_submits_a_line() {
        assert!(submits_a_line(b"\r"));
        assert!(submits_a_line(b"hello\r"));
        assert!(
            submits_a_line(b"\x1b[200~pasted\rtext\x1b[201~\r"),
            "Enter after the paste"
        );
        assert!(!submits_a_line(b"\x1b\r"), "Shift+Enter inserts a newline");
        assert!(
            !submits_a_line(b"\x1b[200~line one\rline two\x1b[201~"),
            "a pasted newline"
        );
        assert!(
            !submits_a_line(b"\x1b[?1;2c\x1b[12;40R\x1b]11;rgb:0000/0000/0000\x1b\\"),
            "terminal replies"
        );
        assert!(!submits_a_line(b"\x1b[I"), "a focus report");
        assert!(!submits_a_line(b""));
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
