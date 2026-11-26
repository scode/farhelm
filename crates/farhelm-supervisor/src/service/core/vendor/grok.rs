//! Grok's stateful supervisor behavior: exact record-pair admission and
//! resume verification, and its proven report admission.

use super::super::*;
use crate::agent_kind::report_pin::{PinDecision, ReportPin};
use crate::hook_report::ReportAdmission;
use crate::service::notifications::{GROK_PIN_CORRECTED_TEXT, NotificationKind};

impl Supervisor {
    /// Grok's half of `verify_report_only_resume`: re-verify the exact
    /// record pair its stored locator names, and invalidate a stale locator
    /// (only the exact locator and generation the caller read) when it no
    /// longer checks out. OS read failures preserve the offer and refuse only
    /// this attempt; missing evidence notifies after durable withdrawal, while
    /// inconsistent evidence withdraws silently.
    pub(in crate::service::core) async fn verify_grok_resume(
        &self,
        session_id: &str,
        snapshot: &SessionSnapshot,
    ) -> anyhow::Result<()> {
        let stored = snapshot.captured_conversation.as_deref().ok_or_else(|| {
            anyhow::anyhow!("a Grok resume offer has no durable conversation locator")
        })?;
        let mut locator = crate::agent_kind::grok::GrokLocator::parse(stored)
            .context("decoding the Grok resume locator")?;
        let verdict = locator.verify_exact_files().await;
        if let Err(error) = &verdict
            && record_read_is_transient(error)
        {
            return Err(RequestError::new(
                ErrorKind::Conflict,
                "the Grok conversation files could not be read; try Restart again",
            )
            .into());
        }
        let definitive = verdict.is_ok();
        locator.resumable = verdict.unwrap_or(false);
        if locator.resume_id().is_some() {
            return Ok(());
        }
        let replacement = locator.encode()?;
        let withdrawn = self
            .store
            .replace_reported_conversation_if_current(
                session_id,
                snapshot.generation,
                Some(stored),
                &replacement,
            )
            .await
            .context("invalidating a stale Grok resume locator")?;
        // Only a clean absence verdict notifies. A different conversation is
        // an inconsistent binding, not an ordinary user deletion.
        if withdrawn && definitive {
            self.notify_session(
                session_id,
                snapshot.generation,
                crate::service::notifications::NotificationKind::ResumeWithdrawn,
                &crate::service::notifications::resume_withdrawn_text("Grok"),
            )
            .await;
        }
        Err(RequestError::new(
            ErrorKind::Conflict,
            "this session's restart offer changed while the restart was being prepared; its \
             exact Grok record pair could not be verified, so nothing was relaunched — \
             refresh the session and re-present the offer",
        )
        .into())
    }

    /// Admit a Grok callback from the pinned reporter or an ancestor that can correct it.
    ///
    /// `SessionStart` is the only selecting event. Its vendor timestamp is
    /// compared while the capture claim is held, which keeps a delayed old
    /// callback from restoring a UUID displaced by `/new`, including after a
    /// supervisor restart. A recorded ancestor's selection has authority over
    /// a child-first choice regardless of timestamps. Enrichment belongs to
    /// the selected UUID and cannot take over from a pinned child.
    pub(in crate::service::core) async fn report_grok_conversation(
        &self,
        id: &str,
        report: ReportedConversation,
        kind: AgentKind,
        generation: i64,
        entry: Option<Arc<SessionEntry>>,
    ) -> Result<ReportAdmission, RequestError> {
        let ReportedConversation {
            vendor: _,
            conversation,
            source,
            transcript_path,
            hook_event_name,
            ancestry,
            launch_generation: _,
        } = report;
        if transcript_path.is_some() {
            return Err(RequestError::new(
                ErrorKind::InvalidRequest,
                "Grok evidence must be carried in its bounded locator",
            ));
        }
        let event = hook_event_name
            .as_ref()
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                RequestError::new(ErrorKind::InvalidRequest, "Grok report has no event name")
            })?;
        if !matches!(event, "SessionStart" | "UserPromptSubmit" | "Stop") {
            return Err(RequestError::new(
                ErrorKind::InvalidRequest,
                "Grok reported an unsupported hook event",
            ));
        }
        if event == "SessionStart" && !matches!(source.as_str(), "new" | "load") {
            return Err(RequestError::new(
                ErrorKind::InvalidRequest,
                "Grok SessionStart requires source new or load",
            ));
        }
        let mut incoming = crate::agent_kind::grok::GrokLocator::parse(&conversation)
            .map_err(|error| RequestError::new(ErrorKind::InvalidRequest, error.to_string()))?;
        if (event == "SessionStart") != incoming.selected_at.is_some() {
            return Err(RequestError::new(
                ErrorKind::InvalidRequest,
                "Grok selection timestamps belong only to SessionStart reports",
            ));
        }
        let chain = ancestry.ok_or_else(|| {
            RequestError::new(
                ErrorKind::Conflict,
                "the Grok report carries no process ancestry reaching this session's pane",
            )
        })?;

        // Keep the ordered selection and foreground proofs under the claim:
        // the reload must remain the binding those proofs and the
        // generation-fenced write agree on. Moving them before the claim
        // would let a refresh change that binding and turn contention into a
        // refusal again. The claim is unbounded because all work under it is
        // local.
        let _capture_claim = self.claim_capture_for_report(id).await;
        let row = self
            .store
            .session(id)
            .await
            .map_err(|_| {
                RequestError::new(ErrorKind::Internal, "could not verify the Grok launch")
            })?
            .ok_or_else(|| {
                RequestError::new(ErrorKind::NotFound, "the Grok session no longer exists")
            })?;
        if row.generation != generation || row.agent_kind() != kind {
            return Err(RequestError::new(
                ErrorKind::Conflict,
                "this session has moved on to another launch",
            ));
        }

        let attribution = crate::procs::grok_corridor(&chain)
            .map_err(|reason| RequestError::new(ErrorKind::Conflict, reason))?;
        let stored_pin = self.store.report_pin(id).await.map_err(|_| {
            RequestError::new(
                ErrorKind::Internal,
                "could not read the Grok reporting-process pin",
            )
        })?;
        let pin = stored_pin
            .as_deref()
            .map(serde_json::from_str::<ReportPin>)
            .transpose()
            .map_err(|_| {
                RequestError::new(
                    ErrorKind::Internal,
                    "the Grok reporting-process pin is invalid",
                )
            })?;
        let decision = super::judge_report_pin(pin.as_ref(), attribution.emitter, &chain, |_| None)
            .map_err(|reason| RequestError::new(ErrorKind::Conflict, reason))?;
        let correction = decision == PinDecision::Correct;
        if event != "SessionStart" && decision != PinDecision::Accept {
            // A turn callback cannot select a different conversation, even from a recorded ancestor.
            return Ok(ReportAdmission::Ignored);
        }
        if decision == PinDecision::Refuse {
            return Err(RequestError::new(
                ErrorKind::Conflict,
                "the Grok report came from a process other than this launch's pinned emitter",
            ));
        }
        if !correction {
            let previous = row
                .captured_conversation
                .as_deref()
                .map(crate::agent_kind::grok::GrokLocator::parse)
                .transpose()
                .map_err(|_| {
                    RequestError::new(
                        ErrorKind::Conflict,
                        "the durable Grok selection cannot be ordered safely",
                    )
                })?;
            incoming =
                crate::agent_kind::grok::GrokLocator::merge_report(previous, incoming, event)
                    .map_err(|error| RequestError::new(ErrorKind::Conflict, error.to_string()))?;
        }
        // A correcting ancestor verifies its own locator. The child's ordering stamp and path
        // belong to the displaced selection, even if both processes report the same UUID.
        incoming.verify().await;
        let conversation = incoming
            .encode()
            .map_err(|error| RequestError::new(ErrorKind::InvalidRequest, error.to_string()))?;
        info!(target: LOG_TARGET,
            session = %id,
            generation,
            emitter_pid = attribution.emitter.pid,
            grok_session = %incoming.session_id,
            resumable = incoming.resumable,
            event,
            "attributed a Grok foreground conversation report"
        );

        let replacement_pin = ReportPin::replacement_json(
            stored_pin.as_deref().zip(pin.as_ref()),
            attribution.emitter,
            attribution.ancestors,
        )
        .map_err(|error| RequestError::new(ErrorKind::Internal, error.to_string()))?;
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
                        1,
                        correction.then_some((
                            NotificationKind::GrokPinCorrected.as_str(),
                            GROK_PIN_CORRECTED_TEXT,
                        )),
                    )
                    .await
            }
        };
        let admission = self.finish_reported_admission(
            id,
            written,
            &conversation,
            &source,
            generation,
            entry,
            1,
        )?;
        // Reload only after the correction and its notice have committed together.
        if correction && self.reload_notification_cell(id).await {
            self.hint_sessions_changed();
        }
        Ok(admission)
    }
}
