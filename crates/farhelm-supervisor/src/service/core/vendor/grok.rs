//! Grok's stateful supervisor behavior: exact record-pair refresh and
//! resume verification, and its proven report admission.

use super::super::*;

impl Supervisor {
    /// Recheck Grok's exact two-file evidence without changing the selected
    /// UUID or its durable ordering timestamp.
    pub(in crate::service::core) async fn refresh_grok_capture_claimed(
        &self,
        row: &mut StoredSession,
    ) -> anyhow::Result<bool> {
        if row.agent_kind() != AgentKind::Grok {
            return Ok(true);
        }
        let Some(current) = self.store.session(&row.id).await? else {
            return Ok(false);
        };
        if current.generation != row.generation || current.agent_kind() != AgentKind::Grok {
            return Ok(false);
        }
        *row = current;
        if row.capture_ownership_version != 1 {
            return Ok(true);
        }
        let Some(stored) = row.captured_conversation.as_deref() else {
            return Ok(true);
        };
        let Ok(mut locator) = crate::agent_kind::grok::GrokLocator::parse(stored) else {
            return Ok(true);
        };
        let was_resumable = locator.resumable;
        let definitive = locator.verify().await;
        if was_resumable == locator.resumable {
            return Ok(true);
        }
        let tell_user = was_resumable && !locator.resumable && definitive;
        if was_resumable && !locator.resumable {
            warn!(target: LOG_TARGET,
                session = %row.id,
                "the exact Grok record pair is unavailable or inconsistent; withdrawing its resume offer"
            );
        }
        let replacement = locator.encode()?;
        if !self.may_record() {
            if !locator.resumable {
                row.captured_conversation = Some(replacement);
            }
            return Ok(true);
        }
        if !self
            .store
            .replace_reported_conversation_if_current(
                &row.id,
                row.generation,
                Some(stored),
                &replacement,
            )
            .await?
        {
            return Ok(false);
        }
        row.captured_conversation = Some(replacement);
        if tell_user {
            // Told to the user once the withdrawal is durable, as Codex's is.
            self.notify_session(
                &row.id,
                row.generation,
                crate::service::notifications::NotificationKind::ResumeWithdrawn,
                &crate::service::notifications::resume_withdrawn_text("Grok"),
            )
            .await;
        }
        Ok(true)
    }

    /// Grok's half of `verify_report_only_resume`: re-verify the exact
    /// record pair its stored locator names, and invalidate a stale locator
    /// (only the exact locator and generation the caller read) when it no
    /// longer checks out.
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
        let definitive = locator.verify().await;
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
        // The final check before a Restart is a withdrawal like the
        // background one, and later refreshes see the offer already gone, so
        // this is the only place the user can be told about it.
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

    /// Admit one manually configured Grok callback through the shared
    /// ownership and capture transaction.
    ///
    /// `SessionStart` is the only selecting event. Its vendor timestamp is
    /// compared while the capture claim is held, which keeps a delayed old
    /// callback from restoring a UUID displaced by `/new`, including after a
    /// supervisor restart. `UserPromptSubmit` and `Stop` can only enrich the
    /// UUID already selected in the durable row.
    pub(in crate::service::core) async fn report_grok_conversation(
        &self,
        id: &str,
        report: ReportedConversation,
        kind: AgentKind,
        generation: i64,
        entry: Option<Arc<SessionEntry>>,
    ) -> Result<(), RequestError> {
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

        let previous = match row.captured_conversation.as_deref() {
            Some(stored) => Some(crate::agent_kind::grok::GrokLocator::parse(stored).map_err(
                |_| {
                    RequestError::new(
                        ErrorKind::Conflict,
                        "the durable Grok selection cannot be ordered safely",
                    )
                },
            )?),
            None => None,
        };
        incoming = crate::agent_kind::grok::GrokLocator::merge_report(previous, incoming, event)
            .map_err(|error| RequestError::new(ErrorKind::Conflict, error.to_string()))?;

        // The recorded chain is already anchored at the current pane; the
        // corridor names its emitter once, from evidence that cannot change
        // while the record pair is verified.
        let emitter = crate::procs::grok_corridor(&chain)
            .map_err(|reason| RequestError::new(ErrorKind::Conflict, reason))?;
        incoming.verify().await;
        let conversation = incoming
            .encode()
            .map_err(|error| RequestError::new(ErrorKind::InvalidRequest, error.to_string()))?;
        info!(target: LOG_TARGET,
            session = %id,
            generation,
            emitter_pid = emitter.pid,
            grok_session = %incoming.session_id,
            resumable = incoming.resumable,
            event,
            "attributed a Grok foreground conversation report"
        );

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
                    .admit_ownership_proven_conversation(
                        id,
                        generation,
                        row.captured_conversation.as_deref(),
                        row.capture_ownership_version,
                        &conversation,
                    )
                    .await
            }
        };
        self.finish_reported_admission(id, written, &conversation, &source, generation, entry, 1)
    }
}
