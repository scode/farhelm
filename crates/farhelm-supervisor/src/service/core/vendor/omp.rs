//! OMP's stateful supervisor behavior: recording each launch's launcher shape
//! and reporter asset, and its proven report admission against them.

use super::super::*;

impl Supervisor {
    /// Tell the user, on the session, that this OMP launch's reports cannot
    /// be accepted because its reporter does not match this build (SPEC.md,
    /// Status).
    ///
    /// Only the two definitive refusals call this: the launch's recorded
    /// reporter is not the current one, or the installed file is there with
    /// contents that differ from this build's. A reporter file that could not
    /// be read, or is missing, may be a passing state and does not notify; a
    /// provenance write that failed at launch does not either, since the
    /// store that refused it would refuse the notification too.
    async fn notify_omp_reporter_mismatch(
        &self,
        row: &StoredSession,
        cause: crate::service::notifications::ReporterMismatch,
    ) {
        self.notify_session(
            &row.id,
            row.generation,
            crate::service::notifications::NotificationKind::ReporterMismatch,
            crate::service::notifications::reporter_mismatch_text(
                cause,
                row.captured_conversation.is_some(),
            ),
        )
        .await;
    }

    /// Record which launcher shape and reporter asset this OMP launch used,
    /// which OMP's admission proof later checks a report against. A failure
    /// is logged and leaves the session runnable without capture.
    pub(in crate::service::core) async fn record_omp_launch(
        &self,
        id: &str,
        generation: i64,
        hooked: bool,
        argv: &[String],
    ) {
        let asset = hooked.then_some(crate::pi_extension::OMP_ASSET.file_name);
        let program = crate::agent_kind::omp::classify_omp_launch(argv).column_value();
        if let Err(error) = self
            .store
            .record_omp_launch_provenance(id, generation, asset, program)
            .await
        {
            warn!(target: LOG_TARGET,
                session = %id,
                error = %format!("{error:#}"),
                "could not record this launch's OMP provenance; \
                 the session stays runnable without capture"
            );
        }
    }

    /// OMP admission: launch provenance plus foreground process attribution,
    /// wired through the shared claim, CAS, and mirror discipline rather
    /// than its own.
    ///
    /// The root leg is a composition, because neither half implies the
    /// asset gate alone. Launch provenance — the session's durable launch
    /// record naming the current gated asset, with the file's bytes
    /// re-verified — establishes that this launch installed the
    /// context-gated reporter. Process attribution — OMP's corridor over the
    /// report's recorded chain, anchored at the current pane — establishes
    /// that THIS reporter descends from that launched runtime.
    /// Together they imply the report passed the asset's interactive-context
    /// gate: a separately launched interactive child passes the gate but
    /// fails attribution, while an old gateless asset fails provenance
    /// before any process is inspected. The session-file header stays a
    /// pre-resume file↔id check rather than ownership evidence, and a
    /// parent lineage field never rejects: legitimate forks carry one.
    pub(in crate::service::core) async fn report_omp_conversation(
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
            transcript_path: _,
            hook_event_name: _,
            ancestry,
            launch_generation: _,
        } = report;
        if !crate::agent_kind::omp::is_omp_foreground_source(&source) {
            return Err(RequestError::new(
                ErrorKind::InvalidRequest,
                "OMP reported an unsupported foreground transition",
            ));
        }
        // Step 1 tail: the cheap shape gate before any file or process I/O
        // — the token must parse as this kind's locator, exactly as the
        // legacy path checks after its reload.
        if !crate::agent_kind::accepts_reported_conversation(kind, &conversation) {
            return Err(RequestError::new(
                ErrorKind::InvalidRequest,
                "the reported conversation identity does not match this session's agent kind",
            ));
        }
        // Step 2: the capture claim, then the authoritative reload
        // and kind/generation comparison — the same ordering the Codex
        // branch reads its binding under.
        // Keep the launch-provenance and foreground-runtime proofs under the
        // claim: the reload must remain the binding those proofs and the
        // generation-fenced write agree on. Moving them before the claim
        // would let a refresh change that binding and turn contention into a
        // refusal again. The claim is unbounded because all work under it is
        // local.
        let _capture_claim = self.claim_capture_for_report(id).await;
        let row = self
            .store
            .session(id)
            .await
            .map_err(|_| RequestError::new(ErrorKind::Internal, "could not verify the OMP launch"))?
            .ok_or_else(|| {
                RequestError::new(ErrorKind::NotFound, "the OMP session no longer exists")
            })?;
        if row.generation != generation || row.agent_kind() != kind {
            return Err(RequestError::new(
                ErrorKind::Conflict,
                "this session has moved on to another launch",
            ));
        }
        let chain = ancestry.ok_or_else(|| {
            RequestError::new(
                ErrorKind::Conflict,
                "the OMP report carries no process ancestry reaching this session's pane",
            )
        })?;
        // Step 3a: launch provenance. The row must name the current gated
        // asset for this launch — a session launched under the old gateless
        // asset carries nothing (or a stale name) and fails closed here,
        // runnable with no capture — and the file must re-read
        // byte-identical to this binary's asset, so a supervisor reload is
        // proven against the current asset rather than trusted.
        if row.omp_reporter_asset.as_deref() != Some(crate::pi_extension::OMP_ASSET.file_name) {
            self.notify_omp_reporter_mismatch(
                &row,
                crate::service::notifications::ReporterMismatch::OlderReporter,
            )
            .await;
            return Err(RequestError::new(
                ErrorKind::Conflict,
                "this session's launch did not install the current OMP reporter; \
                 the report was not recorded",
            ));
        }
        let asset_path = self
            .state_dir
            .join("integrations")
            .join(crate::pi_extension::OMP_ASSET.directory)
            .join(crate::pi_extension::OMP_ASSET.file_name);
        match crate::agent_kind::read_bounded_regular_file(&asset_path).await {
            Ok(Some(text)) if text.as_bytes() == crate::pi_extension::OMP_ASSET.source => {}
            Ok(found) => {
                // Only a file that is there with different contents is a
                // mismatch worth telling the user about; a missing one may be
                // between Farhelm removing and reinstalling it.
                if found.is_some() {
                    self.notify_omp_reporter_mismatch(
                        &row,
                        crate::service::notifications::ReporterMismatch::FileDiffers,
                    )
                    .await;
                }
                return Err(RequestError::new(
                    ErrorKind::Conflict,
                    "the installed OMP reporter does not match this build; \
                     the report was not recorded",
                ));
            }
            Err(error) => {
                warn!(target: LOG_TARGET,
                    session = %id,
                    error = %format!("{error:#}"),
                    "could not verify the installed OMP reporter; the report is discarded"
                );
                return Err(RequestError::new(
                    ErrorKind::Conflict,
                    "the installed OMP reporter could not be verified; \
                     the report was not recorded",
                ));
            }
        }
        // Step 3b: the runtime proof. The RETAINED launch program —
        // classified from the argv this generation actually started, and
        // published beside the asset marker before the launch's first
        // process could exist — selects the installation descriptor;
        // without it there is nothing to bind the recorded chain to. The
        // resume template is a future resume's command and is never
        // consulted here: a supported direct launch with an independent
        // resume override still proves what it runs.
        let program = crate::agent_kind::omp::OmpLaunchProgram::from_column_value(
            row.omp_launch_program.as_deref(),
        );
        // The corridor may resolve an entry spelling on disk, so it runs on
        // the blocking pool; the chain itself is the recorded evidence,
        // already anchored at the current pane, and is read once.
        let emitter =
            tokio::task::spawn_blocking(move || crate::procs::omp_corridor(&chain, &program))
                .await
                // A corridor that panicked would panic again on the same
                // recorded chain, so that refuses rather than asking the
                // report drain to retry it every pass; a cancelled one (only
                // at runtime shutdown) is worth another try.
                .map_err(|error| {
                    RequestError::new(
                        if error.is_panic() {
                            ErrorKind::Conflict
                        } else {
                            ErrorKind::Internal
                        },
                        "OMP process attribution could not complete",
                    )
                })?
                .map_err(|reason| RequestError::new(ErrorKind::Conflict, reason))?;
        info!(target: LOG_TARGET,
            session = %id, generation, emitter_pid = emitter.pid,
            conversation = %conversation, source = %source,
            "attributed an OMP foreground conversation report"
        );
        // Step 4: the atomic CAS over the COMPLETE prior binding,
        // committing identity, locator, provenance 1, source, readiness,
        // and the ambiguity reset together — the same transaction Codex
        // uses, fenced the same way.
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
