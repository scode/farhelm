//! Plain-id report admission with positional attribution and durable authority.
//!
//! Claude and Muse cannot be recognized by executable name. Their shared corridor
//! attributes the hook by position; the first admitted emitter is pinned, so
//! accepting resident wrappers does not let a later shelled-out child replace
//! the conversation.

use super::super::*;

/// Facts selected by per-kind dispatch, separate from the shared authority rules.
/// Both vendors currently emit Stop; keeping the spelling here makes adopting
/// another vendor's end-of-turn policy an explicit decision.
pub(in crate::service::core) struct PositionalReportPolicy {
    kind: AgentKind,
    name: &'static str,
    quiet_event: &'static str,
    correction_kind: crate::service::notifications::NotificationKind,
    correction_text: &'static str,
}

pub(in crate::service::core) const CLAUDE: PositionalReportPolicy = PositionalReportPolicy {
    kind: AgentKind::Claude,
    name: "Claude",
    quiet_event: "Stop",
    correction_kind: crate::service::notifications::NotificationKind::ClaudePinCorrected,
    correction_text: crate::service::notifications::CLAUDE_PIN_CORRECTED_TEXT,
};

/// Muse 1.4.14 reports the parent on SessionStart and Stop; native reminder
/// children use marked SubagentStart/SubagentStop events instead.
pub(in crate::service::core) const MUSE: PositionalReportPolicy = PositionalReportPolicy {
    kind: AgentKind::Muse,
    name: "Muse",
    quiet_event: "Stop",
    correction_kind: crate::service::notifications::NotificationKind::MusePinCorrected,
    correction_text: crate::service::notifications::MUSE_PIN_CORRECTED_TEXT,
};

impl Supervisor {
    /// Admit a positional vendor's conversation without versioned record proof.
    /// The capture claim protects both the recorded process pin and the current
    /// binding. Ancestor correction and its once-per-launch notification commit
    /// together; refusals and failed writes publish nothing. Neither vendor has
    /// an established unmarked same-process child hazard, so this does not read
    /// vendor files and deliberately writes ownership version 0.
    /// Stop is an ordinary selecting report of the current conversation. A
    /// rival Stop acknowledges without effect or warning, including when pin
    /// liveness is unavailable; validation and storage failures remain errors.
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
            ..
        } = report;
        let kind = policy.kind;
        let name = policy.name;
        let stop = hook_event_name.as_ref().and_then(serde_json::Value::as_str)
            == Some(policy.quiet_event);
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
        let attribution = crate::procs::positional_corridor(&chain)
            .map_err(|reason| RequestError::new(ErrorKind::Conflict, reason))?;
        let stored_pin = self.store.report_pin(id).await.map_err(|_| {
            RequestError::new(
                ErrorKind::Internal,
                format!("could not read the {name} reporting-process pin"),
            )
        })?;
        use crate::agent_kind::report_pin::{PinDecision, ReportPin};
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
        let decision = match pin.as_ref() {
            None => PinDecision::Accept,
            Some(pin) => {
                let initial = pin.judge(attribution.emitter, true, &chain);
                if initial == PinDecision::Refuse {
                    // Only positive evidence of exit, PID reuse or a non-running
                    // process permits release. An unreadable process is unknown.
                    // The historical Claude-named test seam exercises this
                    // shared liveness boundary for both positional vendors.
                    let evidence = match self
                        .seams
                        .faults
                        .claude_pin_process_fault()
                        .and_then(|fault| fault(pin.process.pid))
                    {
                        Some(reason) => Err(reason),
                        None => crate::procs::read_process(pin.process.pid),
                    };
                    let live = match evidence {
                        Ok(process) => process.is_some_and(|(_, start, state)| {
                            start == pin.process.start
                                && state == crate::procs::ProcessState::Running
                        }),
                        // A nested agent can report at every turn. Unknown
                        // liveness still retains the pin, but an ordinary Stop
                        // refusal must not become a warning on every turn.
                        Err(_) if stop => return Ok(()),
                        Err(reason) => return Err(RequestError::new(ErrorKind::Conflict, reason)),
                    };
                    pin.judge(attribution.emitter, live, &chain)
                } else {
                    initial
                }
            }
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
        let correction = decision == PinDecision::Correct;
        let replacement_pin = if decision == PinDecision::Accept
            && pin
                .as_ref()
                .is_some_and(|pin| pin.process == attribution.emitter)
        {
            // Preserve correction authority from the winning report even when a
            // later chain ends at a different anchor or loses an ancestor.
            stored_pin
                .clone()
                .expect("the accepted pin was loaded from storage")
        } else {
            serde_json::to_string(&ReportPin {
                process: attribution.emitter,
                ancestors: attribution.ancestors,
            })
            .map_err(|error| RequestError::new(ErrorKind::Internal, error.to_string()))?
        };
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
