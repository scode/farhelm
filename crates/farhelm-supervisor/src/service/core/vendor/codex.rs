//! Codex's stateful supervisor behavior: exact-record refresh of its
//! reported binding, and its proven report admission (foreground-process
//! attribution plus root record validation).

use super::super::*;

impl Supervisor {
    /// Refresh a Codex binding for a caller already holding this session's
    /// capture claim (see above for why the claim cannot be taken twice).
    /// The reload below is still correct under the caller's claim — it
    /// re-reads the row the claim serializes, so a report that landed
    /// between the caller's read and this one is observed, not overwritten.
    pub(in crate::service) async fn refresh_codex_capture_claimed(
        &self,
        row: &mut StoredSession,
    ) -> anyhow::Result<bool> {
        if row.agent_kind() != AgentKind::Codex {
            return Ok(true);
        }
        // The caller may have loaded its row before a report took the claim.
        // Verify the current binding, not an earlier conversation whose file is
        // still valid after a clear. A new launch requires the caller to retry.
        let Some(current) = self.store.session(&row.id).await? else {
            return Ok(false);
        };
        if current.generation != row.generation || current.agent_kind() != AgentKind::Codex {
            return Ok(false);
        }
        *row = current;
        // Provenance this build does not recognize is preserved byte for
        // byte and never promoted: verification below would otherwise
        // acquire a thread for an unknown-version binding and durably
        // rewrite a contract it cannot read. Version 0 flows through
        // deliberately — the v1-token exception keeps pre-column tokens
        // resumable through the existing verifier — and 1 is the version
        // admission writes. Anything else returns before any verify or
        // write; the offer gate independently serves these rows NotCaptured.
        if !matches!(row.capture_ownership_version, 0 | 1) {
            return Ok(true);
        }
        let Some(stored) = row.captured_conversation.as_deref() else {
            return Ok(true);
        };
        let Ok(mut locator) = crate::agent_kind::codex::CodexLocator::parse(stored) else {
            // Legacy identities lack foreground attribution. Keep them intact;
            // the kind's offer/substitution boundary refuses their plain IDs.
            return Ok(true);
        };
        let was_resumable = locator.resumable;
        let had_thread = locator.thread_id.is_some();
        let verified = locator.verify().await;
        if was_resumable && !locator.resumable {
            warn!(target: LOG_TARGET, session = %row.id, "the exact Codex record is unavailable or inconsistent; withdrawing its resume offer");
        }
        // Failure leaves the exact path and any established thread binding
        // intact, but verify has already withdrawn readiness.
        drop(verified);
        if was_resumable == locator.resumable && had_thread == locator.thread_id.is_some() {
            return Ok(true);
        }
        let replacement = locator.encode()?;
        if !self.may_record() {
            // Refusing an unavailable target is safe without a write. Publishing
            // a newly verified identity still requires a durable commitment.
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
        Ok(true)
    }

    /// Codex admission, unchanged from the completed two-proof design: live
    /// foreground native-runtime attribution excluding nested runtimes,
    /// then exact root record validation excluding same-process vendor
    /// threads — wired through the shared claim, CAS, and mirror
    /// discipline rather than its own.
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
            peer,
        } = report;
        if !crate::agent_kind::codex::is_foreground_source(&source) {
            return Err(RequestError::new(
                ErrorKind::InvalidRequest,
                "Codex reported an unsupported foreground transition",
            ));
        }
        let peer = peer.ok_or_else(|| {
            RequestError::new(
                ErrorKind::Conflict,
                "the Codex report has no kernel-attributed local process",
            )
        })?;
        if let Some(gate) = self.seams.faults.codex_report_gate() {
            gate().await;
        }
        // Step 2: the capture claim, then the authoritative reload. A
        // readiness change in the previous conversation must neither discard
        // a legitimate clear nor let a repeated report forget a newly
        // established persistent thread, which is why the binding below is
        // read only after the claim excludes refresh. The foreground and
        // exact-record proofs stay under that same claim so refresh cannot
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
        let mut locator = crate::agent_kind::codex::CodexLocator::reported(
            conversation,
            transcript_path,
            hook_event_name,
        )
        .map_err(|error| RequestError::new(ErrorKind::InvalidRequest, error.to_string()))?;
        // Repeating a report is not permission to rebind an already-known
        // file to another persistent thread. Keep that expectation even
        // after refresh has withdrawn readiness; a legitimate clear/new
        // names a different runtime or path and can establish a new binding.
        if let Some(previous) = row
            .captured_conversation
            .as_deref()
            .and_then(|value| crate::agent_kind::codex::CodexLocator::parse(value).ok())
            && previous.runtime_session_id == locator.runtime_session_id
            && previous.session_file == locator.session_file
        {
            locator.thread_id = previous.thread_id;
        }
        let emitter = self.codex_foreground(&row, peer).await?;
        locator.verify().await.map_err(|error| {
            RequestError::new(
                ErrorKind::InvalidRequest,
                format!("the Codex report's exact record could not be verified: {error}"),
            )
        })?;
        if !locator.resumable && source != "clear" {
            return Err(RequestError::new(
                ErrorKind::Conflict,
                "the Codex report has no persisted root record; the current foreground identity was not changed",
            ));
        }
        if self.codex_foreground(&row, peer).await? != emitter {
            return Err(RequestError::new(
                ErrorKind::Conflict,
                "the Codex foreground changed during verification",
            ));
        }
        info!(target: LOG_TARGET,
            session = %id, generation, emitter_pid = emitter.pid,
            runtime_session = %locator.runtime_session_id,
            persistent_thread = ?locator.thread_id, resumable = locator.resumable,
            "attributed a Codex foreground conversation report"
        );
        conversation = locator
            .encode()
            .map_err(|error| RequestError::new(ErrorKind::InvalidRequest, error.to_string()))?;
        // Step 4: the atomic CAS over the COMPLETE prior binding —
        // generation plus exact locator plus proof version — committing
        // identity, locator, provenance 1, source, readiness, and the
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
            // The capture claim excludes refresh/report interleavings, while
            // the durable precondition also fences lifecycle changes and
            // preserves the exact binding that authorized verification.
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

    /// Recover the agent pane during publication gaps, then bind the socket peer
    /// to its native Codex process. No lifecycle lock: the reporting hook may be
    /// running inside the launch whose publication that lock protects.
    pub(in crate::service::core) async fn codex_foreground(
        &self,
        row: &StoredSession,
        peer: crate::procs::ProcessIdentity,
    ) -> Result<crate::procs::ProcessIdentity, RequestError> {
        let pid = self.owned_pane_pid(row, "Codex").await?;
        tokio::task::spawn_blocking(move || crate::procs::foreground_codex_emitter(peer, pid))
            .await
            .map_err(|_| {
                RequestError::new(
                    ErrorKind::Internal,
                    "Codex process attribution could not complete",
                )
            })?
            .map_err(|reason| RequestError::new(ErrorKind::Conflict, reason))
    }
}
