//! Claude report admission: positional attribution and durable launch authority.
//!
//! Claude cannot be recognized by executable name. Its corridor attributes the
//! hook by position; the first admitted emitter is pinned, so accepting resident
//! wrappers does not let a later shelled-out child replace the conversation.

use super::super::*;

impl Supervisor {
    /// Admit Claude's own conversation without changing its Restart provenance.
    /// The capture claim protects both the recorded process pin and the current
    /// binding. Ancestor correction and its once-per-launch notification commit
    /// together; refusals and failed writes publish nothing. Claude still takes
    /// no transcript proof and writes ownership version 0.
    pub(in crate::service::core) async fn report_claude_conversation(
        &self,
        id: &str,
        report: ReportedConversation,
        kind: AgentKind,
        generation: i64,
        entry: Option<Arc<SessionEntry>>,
    ) -> Result<(), RequestError> {
        let ReportedConversation {
            conversation,
            source,
            ancestry,
            ..
        } = report;
        if !crate::agent_kind::accepts_reported_conversation(kind, &conversation) {
            return Err(RequestError::new(
                ErrorKind::InvalidRequest,
                "the reported conversation identity does not match this session's agent kind",
            ));
        }
        let chain = ancestry.ok_or_else(|| {
            RequestError::new(
                ErrorKind::Conflict,
                "the Claude report carries no process ancestry reaching this session's pane",
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
        let attribution = crate::procs::claude_corridor(&chain)
            .map_err(|reason| RequestError::new(ErrorKind::Conflict, reason))?;
        let stored_pin = self.store.report_pin(id).await.map_err(|_| {
            RequestError::new(
                ErrorKind::Internal,
                "could not read the Claude reporting-process pin",
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
                    "the Claude reporting-process pin is invalid",
                )
            })?;
        let decision = match pin.as_ref() {
            None => PinDecision::Accept,
            Some(pin) => {
                let initial = pin.judge(attribution.emitter, true, &chain);
                if initial == PinDecision::Refuse {
                    // Only positive evidence of exit, PID reuse or a non-running
                    // process permits release. An unreadable process is unknown.
                    let evidence = match self
                        .seams
                        .faults
                        .claude_pin_process_fault()
                        .and_then(|fault| fault(pin.process.pid))
                    {
                        Some(reason) => Err(reason),
                        None => crate::procs::read_process(pin.process.pid),
                    };
                    let live = evidence
                        .map_err(|reason| RequestError::new(ErrorKind::Conflict, reason))?
                        .is_some_and(|(_, start, state)| {
                            start == pin.process.start
                                && state == crate::procs::ProcessState::Running
                        });
                    pin.judge(attribution.emitter, live, &chain)
                } else {
                    initial
                }
            }
        };
        if decision == PinDecision::Refuse {
            return Err(RequestError::new(
                ErrorKind::Conflict,
                "the Claude report came from a process other than this launch's pinned emitter",
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
                        correction.then_some((
                            crate::service::notifications::NotificationKind::ClaudePinCorrected
                                .as_str(),
                            crate::service::notifications::CLAUDE_PIN_CORRECTED_TEXT,
                        )),
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
