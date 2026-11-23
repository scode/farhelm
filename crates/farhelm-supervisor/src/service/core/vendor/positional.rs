//! Plain-id report admission with positional attribution and durable authority.
//!
//! The shared corridor attributes the hook by position across native and
//! wrapper launches. The first admitted emitter is pinned, so
//! accepting resident wrappers does not let a later shelled-out child replace
//! the conversation.

use super::super::*;
use crate::agent_kind::report_pin::{PinDecision, ReportPin};
use crate::procs::PositionalReporter;

/// Evidence a vendor needs beyond the pinned process for a selecting report.
enum RecordCheck {
    None,
    AntigravityMainTranscript,
}

/// Facts selected by per-kind dispatch, separate from the shared authority rules.
/// Built-in vendors currently emit Stop. The public custom reporter has no event selector.
pub(in crate::service::core) struct PositionalReportPolicy {
    kind: AgentKind,
    name: &'static str,
    quiet_event: Option<&'static str>,
    grammar: PositionalReporter,
    record_check: RecordCheck,
    correction_kind: crate::service::notifications::NotificationKind,
    correction_text: &'static str,
}

pub(in crate::service::core) const CLAUDE: PositionalReportPolicy = PositionalReportPolicy {
    kind: AgentKind::Claude,
    name: "Claude",
    quiet_event: Some("Stop"),
    grammar: PositionalReporter::BuiltIn,
    record_check: RecordCheck::None,
    correction_kind: crate::service::notifications::NotificationKind::ClaudePinCorrected,
    correction_text: crate::service::notifications::CLAUDE_PIN_CORRECTED_TEXT,
};

/// Muse 1.4.14 reports the parent on SessionStart and Stop; native reminder
/// children use marked SubagentStart/SubagentStop events instead.
pub(in crate::service::core) const MUSE: PositionalReportPolicy = PositionalReportPolicy {
    kind: AgentKind::Muse,
    name: "Muse",
    quiet_event: Some("Stop"),
    grammar: PositionalReporter::BuiltIn,
    record_check: RecordCheck::None,
    correction_kind: crate::service::notifications::NotificationKind::MusePinCorrected,
    correction_text: crate::service::notifications::MUSE_PIN_CORRECTED_TEXT,
};

/// Antigravity emits both callbacks from the same process for main and child
/// conversations. The transcript check supplies the distinction the pin lacks.
pub(in crate::service::core) const ANTIGRAVITY: PositionalReportPolicy = PositionalReportPolicy {
    kind: AgentKind::Antigravity,
    name: "Antigravity",
    quiet_event: Some("Stop"),
    grammar: PositionalReporter::BuiltIn,
    record_check: RecordCheck::AntigravityMainTranscript,
    correction_kind: crate::service::notifications::NotificationKind::AntigravityPinCorrected,
    correction_text: crate::service::notifications::ANTIGRAVITY_PIN_CORRECTED_TEXT,
};

/// A custom report has only a literal ID and the current launch's positional process evidence.
pub(in crate::service::core) const CUSTOM: PositionalReportPolicy = PositionalReportPolicy {
    kind: AgentKind::Custom,
    name: "custom agent",
    quiet_event: None,
    grammar: PositionalReporter::Custom,
    record_check: RecordCheck::None,
    correction_kind: crate::service::notifications::NotificationKind::CustomPinCorrected,
    correction_text: crate::service::notifications::CUSTOM_PIN_CORRECTED_TEXT,
};

impl Supervisor {
    /// Admit a positional vendor's conversation without versioned record proof.
    /// The capture claim protects both the recorded process pin and the current
    /// binding. Ancestor correction and its once-per-launch notification commit
    /// together after any required vendor record check. The stored binding is
    /// a plain ID with ownership version 0. Stop is a selecting report, but
    /// acknowledgments of rival or unverified Stops leave capture untouched.
    /// Validation and storage failures remain errors.
    pub(in crate::service::core) async fn report_positional_conversation(
        &self,
        id: &str,
        report: ReportedConversation,
        policy: PositionalReportPolicy,
        generation: i64,
        entry: Option<Arc<SessionEntry>>,
    ) -> Result<(), RequestError> {
        let ReportedConversation {
            conversation,
            source,
            ancestry,
            hook_event_name,
            transcript_path,
            ..
        } = report;
        let kind = policy.kind;
        let name = policy.name;
        let stop = policy.quiet_event.is_some_and(|quiet| {
            hook_event_name.as_ref().and_then(serde_json::Value::as_str) == Some(quiet)
        });
        if !crate::agent_kind::accepts_reported_conversation(kind, &conversation) {
            return Err(RequestError::new(
                ErrorKind::InvalidRequest,
                "the reported conversation identity does not match this session's agent kind",
            ));
        }
        let chain = ancestry.ok_or_else(|| {
            RequestError::new(
                ErrorKind::Conflict,
                format!(
                    "the {name} report carries no process ancestry reaching this session's pane"
                ),
            )
        })?;

        // Evidence belongs to the report, not to the hook's remaining lifetime.
        // Reload under the same capture claim used by reconciliation; a relaunch
        // still invalidates the generation before any pin can be adopted.
        let _capture_claim = self.claim_capture_for_report(id).await;
        let row = self
            .store
            .session(id)
            .await
            .map_err(|_| RequestError::new(ErrorKind::Internal, "could not verify the launch"))?
            .ok_or_else(|| {
                RequestError::new(ErrorKind::NotFound, "the session no longer exists")
            })?;
        if row.generation != generation || row.agent_kind() != kind {
            return Err(RequestError::new(
                ErrorKind::Conflict,
                "this session has moved on to another launch",
            ));
        }
        // A registration can be edited or deleted while a session is running. Use the capability
        // stored on this launch under the same capture claim that fences its generation.
        if row.launch.custom_reporting() == Some(false) {
            return Err(RequestError::new(
                ErrorKind::InvalidRequest,
                "conversation reporting is disabled for this launch",
            ));
        }
        let attribution = crate::procs::positional_corridor(&chain, policy.grammar)
            .map_err(|reason| RequestError::new(ErrorKind::Conflict, reason))?;
        let stored_pin = self.store.report_pin(id).await.map_err(|_| {
            RequestError::new(
                ErrorKind::Internal,
                format!("could not read the {name} reporting-process pin"),
            )
        })?;
        let pin = stored_pin
            .as_deref()
            .map(serde_json::from_str::<ReportPin>)
            .transpose()
            .map_err(|_| {
                RequestError::new(
                    ErrorKind::Internal,
                    format!("the {name} reporting-process pin is invalid"),
                )
            })?;
        let decision =
            match super::judge_report_pin(pin.as_ref(), attribution.emitter, &chain, |pid| {
                // The historical Claude seam covers liveness for this entire admission path.
                self.seams
                    .faults
                    .claude_pin_process_fault()
                    .and_then(|fault| fault(pid))
            }) {
                Ok(decision) => decision,
                // A nested agent can report at every turn. Unknown liveness
                // retains the pin, and a quiet Stop need not warn on every turn.
                Err(_) if stop => return Ok(()),
                Err(reason) => return Err(RequestError::new(ErrorKind::Conflict, reason)),
            };
        if decision == PinDecision::Refuse {
            // Returning before the writer and finish helper keeps the report
            // acknowledged without changing capture or logging a refusal.
            if stop {
                return Ok(());
            }
            return Err(RequestError::new(
                ErrorKind::Conflict,
                format!(
                    "the {name} report came from a process other than this launch's pinned emitter"
                ),
            ));
        }
        match policy.record_check {
            RecordCheck::None => {}
            RecordCheck::AntigravityMainTranscript => {
                if !crate::agent_kind::antigravity::verifies_main_conversation(
                    transcript_path.as_ref(),
                )
                .await
                {
                    if stop {
                        return Ok(());
                    }
                    return Err(RequestError::new(
                        ErrorKind::Conflict,
                        "the Antigravity transcript does not verify a main conversation",
                    ));
                }
            }
        }
        let correction = decision == PinDecision::Correct;
        let replacement_pin = ReportPin::replacement_json(
            stored_pin.as_deref().zip(pin.as_ref()),
            attribution.emitter,
            attribution.ancestors,
        )
        .map_err(|error| RequestError::new(ErrorKind::Internal, error.to_string()))?;
        // The fault substitutes for the writer so tests exercise this path's
        // failure handling without making the database unusable for assertions.
        let injected = self
            .seams
            .faults
            .capture_store_fault()
            .as_ref()
            .map(|fault| fault(crate::service::capture::CaptureWrite::Report, id));
        let written = match injected {
            Some(Err(error)) => Err(error),
            _ => {
                self.store
                    .admit_pinned_conversation(
                        &row,
                        stored_pin.as_deref(),
                        &conversation,
                        &replacement_pin,
                        0,
                        correction
                            .then_some((policy.correction_kind.as_str(), policy.correction_text)),
                    )
                    .await
            }
        };
        self.finish_reported_admission(id, written, &conversation, &source, generation, entry, 0)?;
        if correction && self.reload_notification_cell(id).await {
            self.hint_sessions_changed();
        }
        Ok(())
    }
}
