//! Codex's stateful supervisor behavior: click-time verification of its
//! reported binding, and its proven report admission (foreground-process
//! attribution plus root record validation).

use super::super::*;

impl Supervisor {
    /// Check the admitted exact file only when Restart is about to use it.
    /// Missing evidence withdraws durably and notifies; conflicting metadata
    /// withdraws silently. An I/O failure refuses this attempt while retaining
    /// the offer, since no background verifier will restore a transient loss.
    pub(in crate::service::core) async fn verify_codex_resume(
        &self,
        session_id: &str,
        snapshot: &SessionSnapshot,
    ) -> anyhow::Result<()> {
        let stored = snapshot
            .captured_conversation
            .as_deref()
            .context("a Codex resume offer has no durable locator")?;
        let mut locator = crate::agent_kind::codex::CodexLocator::parse(stored)?;
        let verdict = locator.verify().await;
        if let Err(error) = &verdict
            && record_read_is_transient(error)
        {
            return Err(RequestError::new(
                ErrorKind::Conflict,
                "the Codex transcript could not be read; try Restart again",
            )
            .into());
        }
        if locator.resumable {
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
            .await?;
        if withdrawn && verdict.is_ok() {
            self.notify_session(
                session_id,
                snapshot.generation,
                crate::service::notifications::NotificationKind::ResumeWithdrawn,
                &crate::service::notifications::resume_withdrawn_text("Codex"),
            )
            .await;
        }
        Err(RequestError::new(ErrorKind::Conflict,
            "the exact Codex transcript is missing or inconsistent; nothing was relaunched — refresh the session").into())
    }

    /// Admit a verified record only for this launch's pinned Codex process.
    /// First admission pins; a recorded Codex ancestor can correct a child-first
    /// pin, while a dead pin releases for a fresh process outside its chain.
    /// The capture claim and durable CAS keep process authority and the exact
    /// conversation binding together across reports, reloads and relaunches.
    pub(in crate::service::core) async fn report_codex_conversation(
        &self,
        id: &str,
        report: ReportedConversation,
        kind: AgentKind,
        generation: i64,
        entry: Option<Arc<SessionEntry>>,
    ) -> Result<(), RequestError> {
        let ReportedConversation {
            vendor: _,
            mut conversation,
            source,
            transcript_path,
            hook_event_name,
            ancestry,
            launch_generation: _,
        } = report;
        if !crate::agent_kind::codex::is_foreground_report(
            &source,
            hook_event_name.as_ref().and_then(serde_json::Value::as_str),
        ) {
            return Err(RequestError::new(
                ErrorKind::InvalidRequest,
                "Codex reported an unsupported foreground transition",
            ));
        }
        let chain = ancestry.ok_or_else(|| {
            RequestError::new(
                ErrorKind::Conflict,
                "the Codex report carries no process ancestry reaching this session's pane",
            )
        })?;
        if let Some(gate) = self.seams.faults.codex_report_gate() {
            gate().await;
        }
        // Step 2: the capture claim, then the authoritative reload. A
        // readiness change in the previous conversation must neither discard
        // a legitimate clear nor let a repeated report forget a newly
        // established persistent thread, which is why the binding below is
        // read only after the claim excludes competing reports. The foreground and
        // exact-record proofs stay under that same claim so a competing report cannot
        // change the binding between either proof and the generation-fenced
        // write; moving them out would turn contention into a refusal again.
        // The claim is unbounded because all work under it is local.
        let _capture_claim = self.claim_capture_for_report(id).await;
        let row = self
            .store
            .session(id)
            .await
            .map_err(|_| {
                RequestError::new(ErrorKind::Internal, "could not verify the Codex launch")
            })?
            .ok_or_else(|| {
                RequestError::new(ErrorKind::NotFound, "the Codex session no longer exists")
            })?;
        if row.generation != generation || row.agent_kind() != kind {
            return Err(RequestError::new(
                ErrorKind::Conflict,
                "this session has moved on to another launch",
            ));
        }
        let enrichment =
            hook_event_name.as_ref().and_then(serde_json::Value::as_str) == Some("Stop");
        // Attribution must precede Stop's confirmation-only returns: a recorded
        // ancestor's Stop is a selection of its own record, not an enrichment
        // of the child conversation we previously followed by mistake.
        let attribution = crate::procs::codex_corridor(&chain)
            .map_err(|reason| RequestError::new(ErrorKind::Conflict, reason))?;
        let stored_pin = self.store.codex_report_pin(id).await.map_err(|_| {
            RequestError::new(
                ErrorKind::Internal,
                "could not read the Codex reporting-process pin",
            )
        })?;
        let pin = stored_pin
            .as_deref()
            .map(serde_json::from_str::<crate::agent_kind::codex::CodexReportPin>)
            .transpose()
            .map_err(|_| {
                RequestError::new(
                    ErrorKind::Internal,
                    "the Codex reporting-process pin is invalid",
                )
            })?;
        use crate::agent_kind::codex::PinDecision;
        let decision = match pin.as_ref() {
            None => PinDecision::Accept,
            Some(pin) => {
                let initial = pin.judge(&attribution, true, &chain);
                if initial == PinDecision::Refuse {
                    // Unavailable evidence is not proof that the pin exited.
                    // Only a gone/reused/non-running process releases authority.
                    let live = crate::procs::read_process(pin.process.pid)
                        .map_err(|reason| RequestError::new(ErrorKind::Conflict, reason))?
                        .is_some_and(|(_, start, state)| {
                            start == pin.process.start
                                && state == crate::procs::ProcessState::Running
                        });
                    pin.judge(&attribution, live, &chain)
                } else {
                    initial
                }
            }
        };
        if decision == PinDecision::Refuse {
            if enrichment {
                return Ok(());
            }
            return Err(RequestError::new(
                ErrorKind::Conflict,
                "a Codex process below or different from the pinned process cannot report for this launch",
            ));
        }
        let correction = decision == PinDecision::Correct;
        let mut locator = crate::agent_kind::codex::CodexLocator::reported(
            conversation,
            transcript_path,
            hook_event_name,
        )
        .map_err(|error| RequestError::new(ErrorKind::InvalidRequest, error.to_string()))?;
        if enrichment && !correction {
            // The pinned process's Stop is confirmation. An already-ready binding
            // is rejected here before any transcript read, including after its
            // file disappears. Only Restart decides that offer is stale.
            let Some(previous) = row
                .captured_conversation
                .as_deref()
                .and_then(|value| crate::agent_kind::codex::CodexLocator::parse(value).ok())
            else {
                return Ok(());
            };
            if row.capture_ownership_version != 1
                || previous.resumable
                || previous.thread_id.is_some()
                || previous.runtime_session_id != locator.runtime_session_id
            {
                return Ok(());
            }
            if let Some(path) = previous.session_file.as_ref() {
                if locator
                    .session_file
                    .as_ref()
                    .is_some_and(|incoming| incoming != path)
                {
                    return Err(RequestError::new(
                        ErrorKind::Conflict,
                        "the Codex confirmation names a different transcript",
                    ));
                }
                locator.session_file = Some(path.clone());
            }
            locator.thread_id = previous.thread_id;
        }
        // Repeating a report is not permission to rebind an already-known
        // file to another persistent thread. Keep that expectation even
        // after Restart has withdrawn readiness; a legitimate clear/new
        // names a different runtime or path and can establish a new binding.
        if !correction
            && let Some(previous) = row
                .captured_conversation
                .as_deref()
                .and_then(|value| crate::agent_kind::codex::CodexLocator::parse(value).ok())
            && previous.runtime_session_id == locator.runtime_session_id
            && previous.session_file == locator.session_file
        {
            locator.thread_id = previous.thread_id;
        }
        locator.verify().await.map_err(|error| {
            RequestError::new(
                ErrorKind::InvalidRequest,
                format!("the Codex report's exact record could not be verified: {error}"),
            )
        })?;
        if !locator.resumable && enrichment {
            return Ok(());
        }
        if !locator.resumable && source != "clear" {
            return Err(RequestError::new(
                ErrorKind::Conflict,
                "the Codex report has no persisted root record; the current foreground identity was not changed",
            ));
        }
        info!(target: LOG_TARGET,
            session = %id, generation, emitter_pid = attribution.emitter.pid,
            runtime_session = %locator.runtime_session_id,
            persistent_thread = ?locator.thread_id, resumable = locator.resumable,
            "attributed a Codex foreground conversation report"
        );
        conversation = locator
            .encode()
            .map_err(|error| RequestError::new(ErrorKind::InvalidRequest, error.to_string()))?;
        let replacement_pin = if decision == PinDecision::Accept
            && pin
                .as_ref()
                .is_some_and(|pin| pin.process == attribution.emitter)
        {
            // A repeated report must not erase ancestors recorded by the winning
            // report merely because later process evidence ends at another anchor.
            stored_pin
                .clone()
                .expect("the accepted pin was loaded from storage")
        } else {
            serde_json::to_string(&crate::agent_kind::codex::CodexReportPin::from_attribution(
                &attribution,
            ))
            .map_err(|error| RequestError::new(ErrorKind::Internal, error.to_string()))?
        };
        // Step 4: the atomic CAS over the COMPLETE prior binding —
        // generation plus exact locator plus proof version plus process pin — committing
        // identity, locator, pin, provenance 1, source, readiness, and the
        // ambiguity reset together. Only this transaction may
        // establish version 1.
        //
        // The injected failure STANDS IN for the store call rather than
        // preceding it, so a test can exercise this function's own failure
        // path without a store that is genuinely broken.
        let injected = self
            .seams
            .faults
            .capture_store_fault()
            .as_ref()
            .map(|fault| fault(crate::service::capture::CaptureWrite::Report, id));
        let written = match injected {
            Some(Err(e)) => Err(e),
            // The capture claim excludes report/Restart interleavings, while
            // the durable precondition also fences lifecycle changes and
            // preserves the exact binding that authorized verification.
            _ => {
                self.store
                    .admit_codex_conversation(
                        &row,
                        stored_pin.as_deref(),
                        &conversation,
                        &replacement_pin,
                        correction.then_some((
                            crate::service::notifications::NotificationKind::CodexPinCorrected
                                .as_str(),
                            crate::service::notifications::CODEX_PIN_CORRECTED_TEXT,
                        )),
                    )
                    .await
            }
        };
        self.finish_reported_admission(id, written, &conversation, &source, generation, entry, 1)?;
        // The warning committed with the correction. A crash before this mirror
        // only delays its visibility: the next supervisor reloads the durable row.
        if correction && self.reload_notification_cell(id).await {
            self.hint_sessions_changed();
        }
        Ok(())
    }
}
