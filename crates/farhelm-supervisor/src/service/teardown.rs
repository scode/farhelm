//! Whole-session teardown: everything `DeleteSession` does to make a
//! session stop existing, with nothing about how the request is answered.
//!
//! Delete is the slowest and least reversible thing this supervisor does,
//! and its steps are ordered against each other for reasons that are not
//! recoverable from reading any one of them: uploads are cancelled before
//! the multi-second process sweep so nothing goes on writing into a
//! directory that is about to vanish; tab and past-launch scope units are
//! enumerated from the manager because a tmux server that died first leaves
//! no windows to read tab ids from while a scrubbed daemon keeps running
//! inside its cgroup; the removable artifacts go before the DB row because
//! a leftover launch spec holds credentials and this is the last moment
//! anything comes back for it; and the row goes last because a crash that
//! leaves a listed-but-dead session is recoverable while an unlisted-but-
//! running agent is not (lore/2026-07-27-m2-process-tree-stop.md). Keeping
//! the whole sequence in one function is what keeps that ordering
//! reviewable as a unit instead of interleaved with reply plumbing.
//!
//! What this module deliberately does NOT own is the REQUEST: no reply
//! channel reaches it, and no reply message is built here. Failures come
//! back as [`TeardownError`] variants that the handler maps one-for-one
//! onto the error replies it has always sent, which is what lets the
//! fail-closed sequencing above be read (and, in time, reused) without a
//! socket in the picture.
//!
//! Detach notices are the deliberate exception, and the reason is
//! ordering rather than layering. Every notice this teardown makes
//! necessary is INITIATED while the attachment-map guard is still held —
//! that guard-held enqueue IS the atomicity boundary. A concurrent
//! `Attach` for this session is parked on the very same mutex, so
//! enqueueing first is what puts the old client's `Detached` into its
//! queue ahead of anything the racing attach can enqueue; hand the notices
//! back to the handler to send after the guard drops and that attach slips
//! in between, telling a client it is attached to a session another client
//! has not yet been told is gone. `notify_detached` is synchronous and
//! non-blocking, so holding the guard across it costs nothing.
//!
//! "Initiated", precisely, and not "delivered": `notify_detached` is a
//! `try_send` that falls back to a SPAWNED send when that attachment's
//! queue is full. So the enqueue ORDER this establishes is a real
//! guarantee only for queues with room; a saturated one defers its notice
//! to a task that may land after the delete has already been reported.
//! That is the pre-existing behavior of every detach notice in this
//! supervisor, not something the teardown boundary introduces — but it
//! does mean the ordering argument above is about what this module hands
//! to the writer, never about what a client observes under backpressure.
//!
//! This does not weaken the no-reply-channel rule: a notice rides the
//! per-ATTACHMENT `notify` sender that this module already holds by having
//! torn the attachment out of the map, never the request's own reply
//! channel. The handler still owns every byte that answers the
//! `DeleteSession` itself.
//!
//! ## Archive is its own teardown
//!
//! Archive shares the process and terminal reach of delete, but not its
//! retention story. It cancels in-flight uploads, reaps the agent and every
//! tab, removes terminal-only launch artifacts, and detaches
//! every viewer. It then KEEPS the database row and committed attachment
//! directory, marks the row archived, and records the deliberate teardown
//! as `Exited` with `STOP_ANNOTATION` only when it actually stopped a live
//! agent. If the pane was already dead, archive retains the last witnessed
//! outcome instead; treating that case as a fresh user stop would discard an
//! exit code or an error detail that the supervisor already knows.

use super::connection::notify_detached;
use super::core::{ArchiveStage, SessionEntry, Supervisor, unknown_pane_owner_refusal};
use super::launch_artifacts::remove_launch_artifacts_for_session;
use super::status::session_status;
use super::sweep::{ScopeKillFailure, ScopeUnits, SweepTarget, reap_process_tree};
use super::terminals::{ActiveAttach, AttachmentKey};
use super::ticker::ActivitySample;
use super::uploads::abort_session_uploads;
use crate::store::LastOutcome;
use crate::tmux::PaneProbe;

use farhelm_proto::STOP_ANNOTATION;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, warn};

/// Every way a teardown can fail before the session is gone, as one
/// variant per failure CLASS — not per failure site. The four probe/sweep
/// variants each cover exactly one step of the teardown (whatever number
/// of ways that step has of failing); [`TeardownError::FailClosed`]
/// covers the four steps inside the fail-closed block, which is sound
/// because those four already render their own site-specific message and
/// this variant carries it through verbatim.
///
/// Rendering deliberately lives with the handler rather than here: these
/// are wire-visible strings, and keeping them at the boundary that owns
/// the wire is what makes "the error text did not change" something a
/// reader can check in one place.
///
/// Every one of them is fail-closed, and what that does and does not buy
/// is worth being exact about. The DB row survives, so the session is
/// still listed and a LATER DELETE can pick up where this one stopped —
/// that retry is the only thing that finishes the deletion and removes the
/// row. Startup reconciliation is not a fallback here: no delete INTENT is
/// persisted anywhere, so a restart has no way to learn that a delete was
/// ever attempted. What startup does clean up is debris it can recognize
/// on its own (a leftover launch spec, an orphaned scope, a quarantined
/// attachment directory), which bounds the mess a failed teardown leaves
/// behind without ever completing it.
pub(crate) enum TeardownError {
    /// The agent's pane could not be resolved into a trustworthy root pid:
    /// either tmux could not be asked at all, or it answered that the pane
    /// belongs to a tmux session this supervisor does not recognize — a
    /// possible rename or move of this session's own live terminal, which
    /// a delete must not act on (see
    /// [`Supervisor::known_session_tmux_name`]).
    PaneProbe(anyhow::Error),
    /// tmux could not be asked for this session's tabs. Strictly a
    /// failure: see the call site for why "we could not ask" must not
    /// collapse into "there are none".
    TabRediscovery(anyhow::Error),
    /// A systemd user manager exists but would not enumerate this
    /// session's tab or launch-generation scopes.
    TabScopeEnumeration(anyhow::Error),
    /// The process-tree sweep itself failed — not "nothing was found to
    /// kill", but "this could not be confirmed".
    Sweep(anyhow::Error),
    /// The fail-closed block failed — the tmux kill, the launch-artifact
    /// removal, the attachment quarantine, or the row deletion and
    /// reservation settlement.
    ///
    /// The one variant that does NOT carry a source error: those four
    /// sites render their own message as they fail (each needs different
    /// context — which artifact, which step), and this carries that
    /// already-rendered string through unchanged so the reply still names
    /// the step rather than a generic "teardown failed".
    FailClosed(String),
}

/// Failures that prevent archive from truthfully claiming the session is
/// terminal-less and archived.
///
/// Every variant is fail-closed: the archived flag is written only after
/// the process sweep and tmux teardown have succeeded. The handler owns
/// rendering because these messages are part of the wire contract.
///
/// The variants mirror [`TeardownError`]'s one-for-one and carry the same
/// meanings; see that type for what each failure class covers.
pub(crate) enum ArchiveError {
    PaneProbe(anyhow::Error),
    TabRediscovery(anyhow::Error),
    TabScopeEnumeration(anyhow::Error),
    Sweep(anyhow::Error),
    FailClosed(String),
}

impl Supervisor {
    /// Shut down an entire session while preserving its durable metadata,
    /// committed attachments, and any already-known outcome.
    ///
    /// The caller holds the session lifecycle claim across this function.
    /// The archive flag is committed only after every process, tab, tmux
    /// terminal, and terminal-only artifact is gone; a failure therefore
    /// leaves an ordinary visible session that can be retried. Committed
    /// attachment files are never moved or removed. The outcome is replaced
    /// with an annotated exit only when the pane probe found a live owned
    /// agent and this teardown killed it; an already-ended session keeps its
    /// exit code, annotation, or error detail.
    pub(crate) async fn teardown_for_archive(
        &self,
        entry: &SessionEntry,
        session_id: &str,
    ) -> Result<Arc<SessionEntry>, ArchiveError> {
        // The in-memory entry deliberately has no `Terminal` during a
        // restart gap, but the durable tmux name exists for the whole row's
        // lifetime. Archive must still kill a same-named husk in that state;
        // tying the whole-session kill to the agent pane would leave tabs or
        // a dead retained window behind while publishing a terminal-less
        // archive.
        let tmux_name = self
            .store
            .session(session_id)
            .await
            .map_err(|error| {
                ArchiveError::FailClosed(format!(
                    "reading the durable tmux name before archive: {error:#}"
                ))
            })?
            .ok_or_else(|| {
                ArchiveError::FailClosed(format!(
                    "session {session_id} vanished before its tmux terminal could be removed"
                ))
            })?
            .tmux_name;

        let live_pane = match entry.terminal.as_ref() {
            Some(terminal) => {
                if let Some(gate) = &self.seams.archive_gate {
                    gate(ArchiveStage::PaneProbe)
                        .await
                        .map_err(ArchiveError::PaneProbe)?;
                }
                match self
                    .tmux
                    .pane_process(&terminal.tmux_name, &terminal.pane)
                    .await
                    .map_err(ArchiveError::PaneProbe)?
                {
                    PaneProbe::Owned(pane) => Some(pane),
                    PaneProbe::Gone => None,
                    // Same treatment delete gives it, and for the same
                    // reason. A RECOGNIZED owner means the recorded pane
                    // predates the current tmux server, so there is no
                    // root pid here and nothing left of that terminal to
                    // clean — the marker sweep and the per-tab cgroup
                    // scopes reaped below need no pane, and refusing here
                    // is what wedged archive in the 2026-08-16 incident.
                    // An UNRECOGNIZED owner fails closed: the pane may be
                    // this session's own live terminal under a renamed
                    // tmux session, and archiving it would publish a
                    // terminal-less archive while that terminal, its
                    // scrollback, and its tabs went on existing.
                    PaneProbe::ForeignOwner { owner } => {
                        if !self.known_session_tmux_name(&owner).await {
                            return Err(ArchiveError::PaneProbe(anyhow::anyhow!(
                                unknown_pane_owner_refusal(
                                    &terminal.pane,
                                    &owner,
                                    &terminal.tmux_name
                                )
                            )));
                        }
                        warn!(
                            session = %session_id, foreign_owner = %owner,
                            "this session's recorded pane now belongs to another tmux session; \
                             archiving on the marker sweep and cgroup scopes alone"
                        );
                        None
                    }
                }
            }
            None => None,
        };
        let root_pid = live_pane.filter(|pane| !pane.dead).map(|pane| pane.pid);
        let stopped_live_agent = root_pid.is_some();

        // Archive reaches the same whole-session ownership boundary as
        // delete: tabs carry separate cgroup units, and the manager is the
        // only source left when tmux died before its scrubbed daemon did.
        let mut units = ScopeUnits::recorded(entry.scope.clone());
        if let Some(terminal) = entry.terminal.as_ref() {
            if let Some(gate) = &self.seams.archive_gate {
                gate(ArchiveStage::TabRediscovery)
                    .await
                    .map_err(ArchiveError::TabRediscovery)?;
            }
            let tabs = self
                .session_tabs_including_dead(terminal)
                .await
                .map_err(ArchiveError::TabRediscovery)?;
            units.extend_derived(
                tabs.iter()
                    .filter_map(|tab| crate::scope::tab_unit_name(session_id, &tab.id)),
            );
        }
        let globs = [
            crate::scope::tab_unit_glob(session_id),
            crate::scope::launch_unit_glob(session_id),
        ];
        if globs.iter().any(Option::is_some) {
            if let Some(gate) = &self.seams.archive_gate {
                gate(ArchiveStage::ScopeEnumeration)
                    .await
                    .map_err(ArchiveError::TabScopeEnumeration)?;
            }
            for glob in globs.into_iter().flatten() {
                match self.seams.scopes.units_matching(&glob).await {
                    Ok(found) => units.extend_derived(found),
                    Err(error) if !self.seams.scopes.available().await => debug!(
                        session = %session_id,
                        error = %format!("{error:#}"),
                        "no systemd user manager to enumerate this archived session's scopes; \
                         the process-tree sweep is the whole mechanism"
                    ),
                    Err(error) => return Err(ArchiveError::TabScopeEnumeration(error)),
                }
            }
        }
        units.normalize();
        if let Some(gate) = &self.seams.archive_gate {
            gate(ArchiveStage::Sweep)
                .await
                .map_err(ArchiveError::Sweep)?;
        }
        // Everything above is a read-only preflight. Keep transfers alive
        // until those checks have proved teardown can start: a refused
        // archive must not discard an upload and then claim nothing changed.
        // Once the checks pass, cancelling and joining immediately before
        // the first process kill ends the async transfer tasks. An abandoned
        // blocking publication may still complete; its attachment is retained
        // just like one whose path was acknowledged.
        abort_session_uploads(self, session_id, "the session was archived", false).await;
        reap_process_tree(
            &self.seams.scopes,
            units,
            root_pid,
            session_id,
            &SweepTarget::WholeSession,
            ScopeKillFailure::Refuse,
        )
        .await
        .map_err(ArchiveError::Sweep)?;

        // From here through publication, the attachment-map guard prevents
        // a racing attach from installing a viewer on the terminal being
        // removed. Notices are initiated before the guard drops, matching
        // the ordering guarantee described in the module docs.
        let mut attachments = self.attachments.lock().await;
        let doomed: Vec<(AttachmentKey, ActiveAttach)> = attachments
            .extract_if(|key, _| key.session == session_id)
            .collect();
        for (key, old) in &doomed {
            self.begin_forwarder_shutdown(key.clone(), old);
        }
        let mut notify_detach = Vec::with_capacity(doomed.len());
        let mut forwarders = tokio::task::JoinSet::new();
        for (key, old) in doomed {
            let ActiveAttach {
                channel,
                notify,
                forwarder,
                sink,
                ..
            } = old;
            forwarders.spawn(async move {
                let joined = forwarder.await;
                drop(sink);
                (key, joined, channel, notify)
            });
        }
        let mut forwarder_error = None;
        while let Some(joined) = forwarders.join_next().await {
            match joined {
                Ok((key, result, channel, notify)) => {
                    if let Err(error) = self.record_forwarder_join(key, result) {
                        forwarder_error.get_or_insert(error.to_string());
                    }
                    notify_detach.push((channel, notify));
                }
                Err(join) => {
                    forwarder_error
                        .get_or_insert_with(|| format!("terminal cleanup wrapper failed: {join}"));
                }
            }
        }
        if forwarder_error.is_none() && self.has_output_reap_for_session(session_id) {
            forwarder_error = Some(
                "a terminal-output client is still crossing its safe shutdown boundary".to_string(),
            );
        }

        let teardown: Result<(), String> = async {
            if let Some(error) = forwarder_error {
                return Err(error);
            }
            self.tmux
                .kill_session(&tmux_name)
                .await
                .map_err(|error| format!("killing tmux session: {error:#}"))?;
            if let Some(gate) = &self.seams.archive_gate {
                gate(ArchiveStage::ArtifactRemoval)
                    .await
                    .map_err(|error| format!("removing archive artifacts: {error:#}"))?;
            }
            // Launch specs can contain credentials, and a spec is not
            // metadata an archive promises to retain.
            remove_launch_artifacts_for_session(&self.state_dir, session_id).await?;
            self.store
                .archive_session(session_id, stopped_live_agent)
                .await
                .map_err(|error| format!("recording the archived session: {error:#}"))?
                .ok_or_else(|| {
                    format!(
                        "session {session_id} vanished before its archive metadata could be recorded"
                    )
                })?;
            Ok(())
        }
        .await;

        if let Err(message) = teardown {
            for (channel, notify) in &notify_detach {
                notify_detached(
                    notify,
                    *channel,
                    format!("detached during a failed archive: {message}"),
                );
            }
            drop(attachments);
            return Err(ArchiveError::FailClosed(message));
        }

        let prior_outcome = entry
            .outcome
            .lock()
            .expect("outcome mutex poisoned")
            .clone();
        let outcome = if stopped_live_agent && !prior_outcome.is_terminal() {
            LastOutcome::Exited {
                exit_code: None,
                annotation: Some(STOP_ANNOTATION.to_string()),
            }
        } else {
            prior_outcome
        };
        let mut info = entry.info.clone();
        info.archived = true;
        info.tabs.clear();
        let mut archived = Arc::new(SessionEntry {
            info,
            terminal: None,
            outcome: Arc::new(std::sync::Mutex::new(outcome)),
            snapshot: entry.snapshot.clone(),
            canonical_cwd: entry.canonical_cwd.clone(),
            first_input: Arc::clone(&entry.first_input),
            capture: Arc::clone(&entry.capture),
            // Shared for the same reason `capture` is: archiving replaces
            // the entry without ending the launch's story, so the tripwire
            // must keep pointing at the same cells a tick may already be
            // holding.
            hooked: Arc::clone(&entry.hooked),
            hook_warned: Arc::clone(&entry.hook_warned),
            // Reset live classification but keep an accepted burst's failed
            // durable write retryable after this session becomes archived.
            activity: ActivitySample::replacement(&entry.activity),
            last_work_started_at: Arc::clone(&entry.last_work_started_at),
            // Shared rather than reset, unlike the sampler cell above:
            // archiving ends the RUN, not the session's history. This cell
            // is session-scoped everywhere (a relaunch shares it too — see
            // its field docs), so a sampling pass still holding the
            // pre-archive entry writes somewhere the archived entry can be
            // read from, which is the right place for an observation made
            // a moment before the teardown.
            last_activity_at: Arc::clone(&entry.last_activity_at),
            generation: entry.generation,
            // Keep the prior launch's scope identity so a restart can run
            // its ordinary leftover sweep. Archive has already emptied it,
            // but losing the identity would weaken that defense after a
            // partial external cleanup.
            scope: entry.scope.clone(),
        });
        // Use the normal classifier for the published wire fields. Archive
        // has no live pane after teardown, but the classifier still carries
        // terminal codes, annotations, errors, and interrupted outcomes by
        // their established precedence.
        let (status, annotation) = session_status(&archived, &HashMap::new());
        let archived_entry = Arc::get_mut(&mut archived).expect("new archive entry is unique");
        archived_entry.info.status = status;
        archived_entry.info.annotation = annotation;
        self.sessions
            .lock()
            .await
            .insert(session_id.to_string(), Arc::clone(&archived));
        for (channel, notify) in &notify_detach {
            notify_detached(notify, *channel, "session archived".to_string());
        }
        drop(attachments);
        Ok(archived)
    }

    /// Tear this session down completely: cancel its transfers, kill
    /// everything it launched, remove its terminal, its files, and its
    /// row.
    ///
    /// Called under the session's lifecycle claim (the caller's, held
    /// across this whole call) — which is what keeps a concurrent restart
    /// from respawning into a tmux session this is mid-way through
    /// destroying, and what keeps a new transfer from staging into the
    /// directory this is about to take away.
    ///
    /// ALSO called under the supervisor's directory admission
    /// (`working_copy_operations`), passed in as `directory_admission`
    /// rather than acquired here. The lock order is directory admission
    /// BEFORE the lifecycle claim (R1.1), so this function must NEVER
    /// acquire the mutex itself — it would order lifecycle → directory
    /// against every create's intent → directory → lifecycle sequence and
    /// form a cycle with a restricted create waiting on this very delete.
    /// The guard is held across the WHOLE teardown, not just the archival
    /// step: the last-reference decision must be made against the same
    /// world the final transaction commits into, and a create admitted in
    /// between could bind a membership after the member count said zero.
    /// This is the one place a slow process sweep runs under directory
    /// admission; the cost is that concurrent fresh creates wait, and the
    /// alternative is archiving a directory a create just attached to.
    ///
    /// The archival contract (Design E): for each working-copy membership
    /// of the deleted session, if OTHER retained member rows remain, the
    /// delete proceeds normally and only the membership is removed. If
    /// this session is the LAST reference, the checkout is archived —
    /// identity verified (missing source = visible cleanup diagnostic and
    /// metadata deletion; a foreign object at the source FAILS CLOSED),
    /// then `archive_move`'s journal/durable/rename sequence — and in the
    /// final SQLite transaction the session row, its reservations, its
    /// memberships, and the zero-member records whose archive outcome
    /// settled all commit together. An `archive_pending` row is
    /// reconciled first (crash recovery), and an unresolved `planned` row
    /// is retired WITHOUT moving the unknown directory. If `archive_move`
    /// fails, the session row is RETAINED with the failure returned —
    /// partial deletion is the accepted product contract — and a retry
    /// re-decides everything. An unrelated unmanaged directory is never
    /// touched: only registry rows this session is a member of are even
    /// considered.
    ///
    /// The attachment-map guard's ENTIRE lifetime is inside this function,
    /// by design: it is taken once the slow phase is over and released
    /// only after the row is gone, so a concurrent `Attach` cannot install
    /// itself mid-teardown. Every detach notice this teardown makes
    /// necessary is therefore INITIATED BEFORE the matching drop of that
    /// guard, on both the success and the fail-closed path. That ordering
    /// is load-bearing, not incidental — see the module docs, including
    /// why this is initiation rather than a delivery promise — and it is
    /// why the notices are sent from here rather than handed back to the
    /// caller: a caller can only ever start them after the guard is
    /// already gone.
    ///
    /// `session_id` is the id the request named; `entry` is the map value
    /// it resolved to. Both are passed rather than one derived from the
    /// other because the caller has already done that lookup and its
    /// failure (no such session) is a different reply than anything here.
    pub(crate) async fn teardown_session(
        &self,
        entry: &SessionEntry,
        session_id: &str,
        _directory_admission: tokio::sync::OwnedMutexGuard<()>,
    ) -> Result<(), TeardownError> {
        // In-flight uploads are cancelled FIRST — before the
        // process sweep, not after it. The sweep is where a delete
        // spends its seconds (a grace period plus several /proc
        // walks), and a transfer left running through it goes on
        // writing into the directory this delete is about to take
        // away, for as long as the sweep lasts. Cancelling first
        // costs nothing (the transfer is doomed either way) and stops
        // further chunks once each task notices. An already-running
        // blocking disk operation can still finish.
        //
        // `abort_session_uploads` waits for the async tasks, not abandoned
        // blocking operations. A late publication can still race the
        // directory teardown below; cancellation itself is not rollback.
        // The lifecycle claim keeps new transfers from staging here
        // (see `stage_upload`).
        abort_session_uploads(self, session_id, "the session was deleted", true).await;

        // The process-tree sweep runs BEFORE any lock is held: it can
        // take seconds (a grace period plus several /proc walks), and
        // holding `attachments` for that long would stall every OTHER
        // session's attach/input behind one slow delete — the map-
        // wide mutex's already-documented coarseness (see the
        // `Supervisor` struct's lock-discipline docs) made worse if a
        // multi-second sweep sat inside it. A concurrent Attach can
        // therefore install a fresh attachment WHILE this runs; the
        // lock-held phase below tears down WHATEVER attachment exists
        // by the time it runs, new or old, and gives it the deleted
        // notice — that is the one acceptable consequence of not
        // holding the lock here, not an oversight.
        //
        // Same dead/absent/terminal-less handling as `StopSession`:
        // the marker sweep still runs even with no live pane pid, for
        // the same leftover-reaping reason documented there.
        let live_pane = match entry.terminal.as_ref() {
            Some(terminal) => match self
                .tmux
                .pane_process(&terminal.tmux_name, &terminal.pane)
                .await
                .map_err(TeardownError::PaneProbe)?
            {
                PaneProbe::Owned(pane) => Some(pane),
                PaneProbe::Gone => None,
                // The headline case of the 2026-08-16 incident: a pane id
                // recycled onto another session used to make this probe a
                // hard error, and delete became impossible for the rest of
                // the supervisor's life. A RECOGNIZED owner means exactly
                // that recycle, so there is no live pane root here and
                // nothing of the old terminal left to remove; the sweep
                // below still reaps by environment marker and through the
                // session's cgroup scopes (enumeration only NAMES those
                // units — `reap_process_tree` is what kills), neither of
                // which needs a pane. Using the stranger's pid instead
                // would reap THEIR process tree, which is the outcome the
                // session scoping exists to prevent.
                //
                // An UNRECOGNIZED owner fails closed, as every verb did
                // before this probe classified: the recorded pane may be
                // this session's own live terminal under a renamed tmux
                // session, and a delete would then kill a live agent
                // without consent and still leave the renamed container,
                // its scrollback, and its tabs behind while reporting
                // success. The row and the map entry survive the refusal,
                // so a retry — or an operator undoing the rename — can
                // finish the job.
                PaneProbe::ForeignOwner { owner } => {
                    if !self.known_session_tmux_name(&owner).await {
                        return Err(TeardownError::PaneProbe(anyhow::anyhow!(
                            unknown_pane_owner_refusal(&terminal.pane, &owner, &terminal.tmux_name)
                        )));
                    }
                    warn!(
                        session = %session_id, foreign_owner = %owner,
                        "this session's recorded pane now belongs to another tmux session; \
                         deleting on the marker sweep and cgroup scopes alone"
                    );
                    None
                }
            },
            None => None,
        };
        let root_pid = live_pane.filter(|pane| !pane.dead).map(|pane| pane.pid);
        // `WholeSession`: delete is the one lifecycle operation
        // that takes tabs down with the agent (SPEC.md — stop
        // leaves them running, delete and archive do not), so this
        // sweep deliberately does NOT subtract tab processes. It
        // needs no per-tab PPID root either: a tab's shell carries
        // the session marker like everything else the session
        // launched, and the marker scan finds it wherever it is.
        // (Rediscovery below only needs the tab's WINDOW, which
        // survives whether or not its shell already exited —
        // `remain-on-exit` keeps a dead pane's window listed — so
        // "the tmux teardown has not run yet" is what matters
        // here, not that any pane is still alive.)
        //
        // Every tab's SCOPE, however, does have to be named: a
        // cgroup kill can only reach what its own `systemd-run`
        // placed there, so the agent's unit alone would leave a
        // tab's environment-scrubbing double-fork behind — the one
        // shape the marker sweep provably cannot find.
        //
        // Named from TWO independent sources, and the second is
        // the load-bearing one. Rediscovering tabs from tmux
        // covers the ordinary case, but the case that matters here
        // is a tmux server that died BEFORE the delete: there are
        // no windows left to read tab ids from, while a scrubbed
        // tab daemon is still running inside a cgroup that
        // outlived its pane. So the manager is also asked directly
        // for every unit matching this session's tab and launch
        // globs, which need no tmux at all. The launch glob reaches
        // prior generations whose failed kill left a scrubbed daemon
        // after the old portable sweep reported clean. A failure to
        // ENUMERATE fails the delete outright, row retained:
        // publishing "deleted" over an unenumerated cgroup is exactly the unreapable,
        // invisible agent lore/2026-07-27-m2-process-tree-stop.md
        // ends on.
        let mut units = ScopeUnits::recorded(entry.scope.clone());
        if let Some(terminal) = entry.terminal.as_ref() {
            // Strict: "we could not ask tmux" is not "there
            // are no tabs", and a delete that assumed the
            // latter would skip live tab scopes.
            let tabs = self
                .session_tabs_including_dead(terminal)
                .await
                .map_err(TeardownError::TabRediscovery)?;
            units.extend_derived(
                tabs.iter()
                    .filter_map(|tab| crate::scope::tab_unit_name(session_id, &tab.id)),
            );
        }
        for glob in [
            crate::scope::tab_unit_glob(session_id),
            crate::scope::launch_unit_glob(session_id),
        ]
        .into_iter()
        .flatten()
        {
            match self.seams.scopes.units_matching(&glob).await {
                Ok(found) => units.extend_derived(found),
                // Only a host with a usable manager can be asked at
                // all; where there is none this is not a failure,
                // it is the sweep-only world M2 already lived in.
                Err(e) if !self.seams.scopes.available().await => debug!(
                    session = %session_id, error = %format!("{e:#}"),
                    "no systemd user manager to enumerate this session's scopes; \
                     the process-tree sweep is the whole mechanism"
                ),
                Err(e) => return Err(TeardownError::TabScopeEnumeration(e)),
            }
        }
        units.normalize();
        reap_process_tree(
            &self.seams.scopes,
            units,
            root_pid,
            session_id,
            &SweepTarget::WholeSession,
            ScopeKillFailure::Refuse,
        )
        .await
        .map_err(TeardownError::Sweep)?;

        // Everything from here on is fast (one tmux round trip, a
        // few fail-closed removals, one sqlite
        // write) and runs under `attachments`, mirroring the Attach
        // handler's takeover for the same reason: a concurrent Attach
        // must not be able to install itself mid-teardown. This is
        // also the one path that acquires BOTH locks at once — `map
        // removal` below briefly takes `sessions` too, while still
        // holding `attachments` — which is the ordering rule this
        // establishes and the only one that needs to exist as long as
        // nothing else ever needs both: `attachments` first,
        // `sessions` second.
        let mut attachments = self.attachments.lock().await;
        // EVERY terminal of the session, not just the agent's: a
        // delete takes the whole session down, so every channel it
        // has attached is about to be streaming something that no
        // longer exists (PLAN_M4.md item 3's session-scoped
        // ownership, on the teardown side). Restart is the
        // deliberate contrast — see `detach_for_restart`.
        //
        // Stop the forwarders now, before they can race their own
        // natural "session terminal ended" Detached against whatever
        // truthful notice this function sends once the real outcome
        // below is known — but do not send that notice yet.
        //
        // ALL of them are signalled before ANY join is started, exactly as
        // the attach takeover does it. Starting joins in the same loop would
        // let a quick forwarder finish while a later one was still streaming
        // and able to emit its own detach.
        let doomed: Vec<(AttachmentKey, ActiveAttach)> = attachments
            .extract_if(|key, _| key.session == session_id)
            .collect();
        for (key, old) in &doomed {
            self.begin_forwarder_shutdown(key.clone(), old);
        }
        let mut notify_detach = Vec::with_capacity(doomed.len());
        // `..` drops each attachment's input client — killing its
        // control-mode process via `kill_on_drop`, like every other
        // teardown path — and its pause sender, which the stopped
        // forwarder can no longer observe anyway.
        let mut forwarders = tokio::task::JoinSet::new();
        for (key, old) in doomed {
            let ActiveAttach {
                channel,
                notify,
                forwarder,
                sink,
                ..
            } = old;
            forwarders.spawn(async move {
                let joined = forwarder.await;
                drop(sink);
                (key, joined, channel, notify)
            });
        }
        let mut forwarder_error = None;
        while let Some(joined) = forwarders.join_next().await {
            match joined {
                Ok((key, result, channel, notify)) => {
                    if let Err(error) = self.record_forwarder_join(key, result) {
                        forwarder_error.get_or_insert(error.to_string());
                    }
                    notify_detach.push((channel, notify));
                }
                Err(join) => {
                    forwarder_error
                        .get_or_insert_with(|| format!("terminal cleanup wrapper failed: {join}"));
                }
            }
        }
        if forwarder_error.is_none() && self.has_output_reap_for_session(session_id) {
            forwarder_error = Some(
                "a terminal-output client is still crossing its safe shutdown boundary".to_string(),
            );
        }

        // Fail-closed and sequenced deliberately: artifacts before the
        // DB row (a leftover launch spec may hold credentials, and
        // this is the last moment anything will ever come back to
        // remove it — see `remove_fail_closed`'s docs), and the row
        // only after the terminal and process tree are positively
        // gone (a crash here leaves a listed-but-dead session,
        // recoverable by the next delete or a manual cleanup, rather
        // than an unlisted-but-running agent, invisible and
        // unreapable — see lore/2026-07-27-m2-process-tree-stop.md's
        // final paragraph). One `Result`-returning block with `?`
        // rather than a hand-threaded `teardown_error` variable, now
        // that none of these steps need to happen outside the lock.
        // Where this session's attachments were parked, if it had
        // any: filled in by the block below and discarded only
        // once the row removal has committed (see the
        // quarantining step's own comment).
        let mut quarantined: Option<PathBuf> = None;
        let teardown: Result<Vec<String>, String> = async {
            if let Some(error) = forwarder_error {
                return Err(error);
            }
            // Reload can deliberately retain a row without a terminal when
            // tmux has not exposed its pane yet. The durable name still
            // identifies the whole session, so delete must use it as the
            // kill target rather than leaving a same-named tmux husk behind.
            // For that terminal-less case the name is only a CANDIDATE: a
            // session whose tmux side never existed (a launch that failed
            // before its window, a supervisor whose private tmux server was
            // never started) must still delete cleanly, so the kill runs
            // only when tmux confirms the session is there. A server that
            // is not running at all is the same answer as "not there".
            let tmux_name = match entry.terminal.as_ref() {
                Some(terminal) => Some(terminal.tmux_name.clone()),
                None => match self.store.tmux_name(session_id).await {
                    Ok(Some(tmux_name)) => match self.tmux.has_session(&tmux_name).await {
                        Ok(true) => Some(tmux_name),
                        Ok(false) => None,
                        // tmux spells "there is no server" two ways depending
                        // on version: `no server running on <path>` and
                        // `error connecting to <path> (No such file or
                        // directory)`. Both mean nothing to kill.
                        Err(error)
                            if error.to_string().contains("no server running")
                                || error.to_string().contains("error connecting to") =>
                        {
                            None
                        }
                        Err(error) => {
                            return Err(format!(
                                "checking whether the durable tmux session still exists before \
                                 delete: {error:#}"
                            ));
                        }
                    },
                    Ok(None) => {
                        return Err(format!(
                            "session {session_id} vanished before its tmux terminal could be removed"
                        ));
                    }
                    Err(error) => {
                        return Err(format!(
                            "reading the durable tmux name before delete: {error:#}"
                        ));
                    }
                },
            };
            if let Some(tmux_name) = tmux_name {
                self.tmux
                    .kill_session(&tmux_name)
                    .await
                    .map_err(|e| format!("killing tmux session: {e:#}"))?;
            }
            // EVERY generation's launch files, not just the current
            // one: they are named per launch now
            // (`launch::spec_path_for_launch`), and a session that
            // was restarted has one pair per launch it ever had.
            // Delete is the last moment anything comes back for
            // them, and a spec holds the agent's full command line
            // — credentials included — so a missed generation is a
            // credential leak, not untidiness. Fail-closed for that
            // reason (`remove_fail_closed`), including the failure
            // to LIST them: an unreadable directory is not evidence
            // there was nothing in it.
            remove_launch_artifacts_for_session(&self.state_dir, session_id).await?;
            // SPEC.md: "attachment files are removed when their
            // session is deleted". DETACHED here (an atomic
            // rename into the reserved quarantine directory) and
            // actually removed after the row is gone, which is
            // what makes the crash window safe in the right
            // direction: this step still fails the delete closed
            // — with the row retained for a retry — while a crash
            // between it and the commit leaves debris the next
            // startup reconciles rather than a live session whose
            // attachments have silently vanished.
            //
            // Nothing recreates the directory afterwards: every
            // in-flight transfer was cancelled and joined above,
            // and a new one cannot start while the caller holds
            // the session's lifecycle claim.
            quarantined =
                crate::attachments::quarantine_session_dir(&self.state_dir, session_id).await?;
            // Design E's archival decision, made HERE — after the process
            // sweep and terminal removal have PROVEN complete, before the
            // final transaction — and under the directory admission the
            // caller passed in (see this method's doc for why the guard is
            // a parameter and never acquired here). Each membership of the
            // dying session is decided per row; the final transaction in
            // `delete_session_archiving_memberships` commits the outcomes
            // atomically with the row deletion.
            for row in self
                .store
                .member_working_copies_all(session_id)
                .await
                .map_err(|e| format!("enumerating the deleted session's checkout memberships: {e:#}"))?
            {
                match row.allocation_state {
                    crate::working_copies::AllocationState::Planned => {
                        // The ambiguous unresolved plan (Design C's crash
                        // window): an explicit Delete retires the record
                        // without moving the unknown directory — no
                        // rename, no adoption. Name the preserved path in
                        // the diagnostic so the operator can find what we
                        // deliberately left alone.
                        let path = PathBuf::from(&row.canonical_root).join(&row.original_basename);
                        if tokio::fs::symlink_metadata(&path).await.is_ok() {
                            warn!(
                                session = %session_id, path = %path.display(),
                                "delete retired an unresolved checkout plan; the directory at \
                                 the recorded path has no established ownership and is left untouched"
                            );
                        }
                    }
                    crate::working_copies::AllocationState::Allocated
                    | crate::working_copies::AllocationState::ArchivePending => {
                        if self
                            .store
                            .working_copy_member_count(&row.id)
                            .await
                            .map_err(|e| format!("counting the checkout's members: {e:#}"))?
                            > 1
                        {
                            // Other retained member rows remain: the
                            // checkout stays exactly where it is.
                            continue;
                        }
                        // Last reference: this delete's row removal would
                        // orphan the checkout, so the archive decision
                        // below governs. First the corrupt-evidence check
                        // (Design E): overlapping managed paths can only
                        // exist against the admission rule, and such a
                        // registry is preserved — the automated move is
                        // refused until an expert has assessed it.
                        let overlapping = self
                            .store
                            .working_copy_rows()
                            .await
                            .map_err(|e| {
                                format!("reading the registry for the reference check: {e:#}")
                            })?
                            .into_iter()
                            .filter(|other| {
                                other.id != row.id
                                    && other.allocation_state
                                        != crate::working_copies::AllocationState::Retired
                                    && other
                                        .canonical_path
                                        .as_deref()
                                        .is_some_and(|other_path| {
                                            crate::working_copies::path_overlaps(&row.canonical_path, other_path)
                                        })
                            })
                            .count();
                        if overlapping > 0 {
                            return Err(format!(
                                "the checkout at {} has another active registry record whose \
                                 path overlaps it; this is inconsistent ownership evidence, \
                                 so the automated archive move is refused and the registry \
                                 is preserved for inspection",
                                row.canonical_path.as_deref().unwrap_or(&row.canonical_root)
                            ));
                        }
                        // A pending row first completes its crash
                        // recovery; a live row is archived.
                        if row.allocation_state
                            == crate::working_copies::AllocationState::ArchivePending
                        {
                            match self
                                .store
                                .reconcile_working_copy_archive(&row.id, self.seams.archive_parent_sync.clone())
                                .await
                                .map_err(|e| {
                                    format!("reconciling the pending archive: {e:#}")
                                })? {
                                crate::working_copies::ReconcileOutcome::Moved { destination } => {
                                    warn!(session = %session_id, destination = %destination,
                                        "the interrupted archive of a deleted session's checkout \
                                         completed on retry");
                                }
                                crate::working_copies::ReconcileOutcome::MetadataComplete => {}
                                crate::working_copies::ReconcileOutcome::SourceMissing => {
                                    warn!(session = %session_id,
                                        "a pending archive's source was already gone; \
                                         deleting the record only");
                                }
                            }
                        } else {
                            // Missing-source cleanup needs the same root
                            // proof as a rename: a replacement empty root can
                            // otherwise hide a still-owned checkout elsewhere.
                            crate::working_copies::verified_root(&row)
                                .map_err(|e| format!("verifying the checkout's recorded root: {e:#}"))?;
                            match crate::working_copies::verify_identity(&row) {
                                Ok(crate::working_copies::IdentityStatus::Missing) => {
                                    // Missing source: a visible cleanup
                                    // diagnostic, and metadata deletion is
                                    // permitted without moving anything.
                                    warn!(session = %session_id,
                                        path = %row.canonical_path.as_deref().unwrap_or(""),
                                        "the managed checkout's recorded directory is gone; \
                                         deleting its ownership record without any move");
                                }
                                Ok(crate::working_copies::IdentityStatus::Matches) => {
                                    match self.store.archive_move_working_copy(&row.id, self.seams.archive_parent_sync.clone()).await {
                                        Ok(crate::working_copies::ArchiveOutcome::Archived {
                                            destination,
                                        }) => {
                                            warn!(session = %session_id, destination = %destination,
                                                "the last reference to this checkout is being \
                                                 deleted; the directory moved to the archive");
                                        }
                                        Ok(crate::working_copies::ArchiveOutcome::SourceMissing) => {
                                            warn!(session = %session_id,
                                                "the checkout vanished between the identity \
                                                 check and the archive move; deleting only \
                                                 the record");
                                        }
                                        // A post-rename durability failure
                                        // retains the row and journal too.
                                        // Do not claim the path stayed put:
                                        // retry reconciles the recorded move.
                                        Err(e) => {
                                            return Err(format!(
                                                "archiving the checkout at {} kept the session \
                                                 row; the directory may already be at its \
                                                 journaled archive destination: {e:#}",
                                                row.canonical_path.as_deref().unwrap_or(""),
                                            ));
                                        }
                                    }
                                }
                                Ok(crate::working_copies::IdentityStatus::DifferentObject) => {
                                    return Err(format!(
                                        "the managed checkout at {} is no longer the object its \
                                         registry row captured; the directory is not ours to \
                                         move and the session is retained",
                                        row.canonical_path.as_deref().unwrap_or(""),
                                    ));
                                }
                                Ok(status) => {
                                    return Err(format!(
                                        "the managed checkout's registry row has no captured \
                                         identity ({status:?}); refusing to move or delete \
                                         evidence, session retained",
                                    ));
                                }
                                Err(e) => {
                                    return Err(format!(
                                        "verifying the managed checkout's identity kept the \
                                         session row and moved nothing: {e:#}"
                                    ));
                                }
                            }
                        }
                    }
                    crate::working_copies::AllocationState::Retired => {}
                }
            }
            // Settles this session's create reservations in the
            // same transaction as the row removal, which is what
            // turns them into TOMBSTONES rather than stale claims:
            // a replay of one of those intent keys must report the
            // gone-error, never a dead id and never a fresh
            // duplicate (PLAN_M3.md item 6; the store method's own
            // docs carry the argument).
            self.store
                .delete_session_archiving_memberships(session_id)
                .await
                .map_err(|e| format!("{e:#}"))
        }
        .await;

        let retired_checkouts = match teardown {
            Ok(retired) => retired,
            Err(err_msg) => {
                for (channel, notify) in &notify_detach {
                    notify_detached(
                        notify,
                        *channel,
                        format!("detached during a failed delete: {err_msg}"),
                    );
                }
                drop(attachments);
                return Err(TeardownError::FailClosed(err_msg));
            }
        };
        self.sessions.lock().await.remove(session_id);
        // Every former member's process teardown has now completed, and the
        // final membership is durably gone. Unlinking a preparation lock any
        // earlier could split a live shim's flock across two different inodes.
        for checkout_id in retired_checkouts {
            cleanup_retired_preparation(&self.state_dir, &checkout_id).await;
        }
        // The row is gone, so the quarantined attachments now
        // belong to nothing and can be removed for real. Failure
        // here is logged rather than reported: there is no row
        // left to retain for a retry, the caller's delete
        // genuinely succeeded, and the next startup reconciles
        // whatever is left.
        if let Some(parked) = quarantined {
            crate::attachments::discard_quarantined(&parked).await;
        }
        // The session's hook trace goes the same way, and in the same
        // best-effort spirit: it is a per-session diagnostic file
        // (`hook_log_path`), it names a session that no longer exists, and
        // nothing about a delete should fail over it. Deliberately NOT
        // fail-closed like the launch specs above — those can hold the
        // user's secrets, whereas this file
        // holds timestamps, conversation ids, and error kinds this
        // supervisor already logs itself. Most sessions have no such file
        // at all (nothing writes one unless the launch was hooked), so a
        // missing path is the ordinary case rather than a surprise.
        let hook_log = crate::service::core::hook_log_path(&self.state_dir, session_id);
        if let Err(e) = tokio::fs::remove_file(&hook_log).await
            && e.kind() != std::io::ErrorKind::NotFound
        {
            warn!(
                session = %session_id, path = %hook_log.display(), error = %e,
                "could not remove a deleted session's conversation-hook trace"
            );
        }

        for (channel, notify) in &notify_detach {
            notify_detached(notify, *channel, "session deleted".to_string());
        }
        drop(attachments);
        Ok(())
    }
}

/// Remove private preparation evidence only after final retirement and process
/// teardown. Cleanup failure cannot undo committed Delete or authorize another
/// checkout move: leave the failed artifact and report its path for inspection.
async fn cleanup_retired_preparation(state_dir: &std::path::Path, checkout_id: &str) {
    // Registry IDs originate as UUIDs. Corrupt persisted text must not turn
    // this metadata cleanup into an unlink outside the preparation directory.
    if uuid::Uuid::parse_str(checkout_id).is_err() {
        warn!(working_copy = %checkout_id, "invalid retired checkout id; preparation files left untouched");
        return;
    }
    let state_path = state_dir
        .join("checkout-preparation")
        .join(format!("{checkout_id}.json"));
    // Keep the state evidence if removing its lock fails. No surviving member
    // can prepare this checkout, so successful unlink needs no replacement lock.
    for path in [
        crate::launch::preparation_lock_path(&state_path),
        state_path,
    ] {
        if let Err(error) = tokio::fs::remove_file(&path).await
            && error.kind() != std::io::ErrorKind::NotFound
        {
            warn!(working_copy = %checkout_id, path = %path.display(), error = %error,
                "checkout retirement committed but preparation cleanup failed; remaining files retained");
            return;
        }
    }
}

/// The archived entry's cell-sharing rule, which has no other coverage:
/// archiving REPLACES a session's entry, and the cells that describe the
/// launch must be the same objects the replaced entry held.
#[cfg(test)]
mod tests {
    use super::super::core::tests::{StateDir, dummy_exe, entry_with, test_admission};
    use super::super::core::{CreateInputs, CreateMode, SupervisorSeams, SupervisorTimeouts};
    use super::*;
    use crate::store::{LastOutcome, StoredSession};

    /// Seed a terminal-less, scoped session so teardown tests can isolate the
    /// cgroup verdict from tmux discovery and pane ownership.
    async fn scoped_session(
        scopes: crate::scope::ScopeManager,
        id: &str,
    ) -> (StateDir, Arc<Supervisor>, Arc<SessionEntry>) {
        let state = StateDir::new();
        let unit = crate::scope::unit_name(id, 0).expect("a UUID id must name a scope unit");
        let sup = Supervisor::new_with_seams(
            state.path(),
            dummy_exe(),
            SupervisorTimeouts::default(),
            SupervisorSeams {
                scopes: Arc::new(scopes),
                ..SupervisorSeams::default()
            },
        )
        .await
        .expect("supervisor");
        sup.store
            .insert_session(
                StoredSession {
                    conversation_source: None,
                    id: id.to_string(),
                    parent: None,
                    archived: false,
                    title: id.to_string(),
                    created_at: 1_700_000_000,
                    last_activity_at: 1_700_000_000,
                    last_work_started_at: 0,
                    creation_seq: 0,
                    cwd: "/tmp".to_string(),
                    invocation: "agent".to_string(),
                    launch: None,
                    tmux_name: format!("fh-{id}"),
                    pane: String::new(),
                    outcome: LastOutcome::Running,
                    agent_kind: farhelm_proto::AgentKind::Generic,
                    resume_template: None,
                    canonical_cwd: None,
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: None,
                    generation: 0,
                    launch_scoped: true,
                    source_profile: None,
                },
                None,
            )
            .await
            .expect("seed the session row");
        let mut entry = entry_with(None, LastOutcome::Running);
        entry.info.id = id.to_string();
        entry.scope = Some(unit);
        let entry = Arc::new(entry);
        sup.sessions
            .lock()
            .await
            .insert(id.to_string(), Arc::clone(&entry));
        (state, sup, entry)
    }

    /// The successful retry manager retires its recorded unit once SIGTERM
    /// has been sent, the way a collected scope of an agent that exits
    /// politely does.
    fn working_scopes() -> crate::scope::ScopeManager {
        crate::scope::ScopeManager::fake_vanishing_after_signal("SIGTERM", Arc::new(|_| {}))
    }

    /// A failed scope kill must block delete without discarding the only row
    /// that can name the scope on a later retry.
    #[farhelm_testtrace::test]
    async fn delete_keeps_a_session_when_scope_kill_fails_and_retries() {
        let id = uuid::Uuid::new_v4().to_string();
        let (state, sup, entry) = scoped_session(
            crate::scope::ScopeManager::fake_failing_kills(Arc::new(|_| {})),
            &id,
        )
        .await;
        assert!(
            sup.store
                .session(&id)
                .await
                .expect("read seeded row")
                .is_some()
        );

        let result = sup
            .teardown_session(&entry, &id, test_admission(&sup).await)
            .await;
        assert!(matches!(result, Err(TeardownError::Sweep(_))));
        assert!(
            sup.store
                .session(&id)
                .await
                .expect("read retained row")
                .is_some(),
            "delete refusal must retain the durable row"
        );
        assert!(
            sup.sessions.lock().await.contains_key(&id),
            "delete refusal must retain the in-memory entry"
        );
        drop(sup);

        let sup = Supervisor::new_with_seams(
            state.path(),
            dummy_exe(),
            SupervisorTimeouts::default(),
            SupervisorSeams {
                scopes: Arc::new(working_scopes()),
                ..SupervisorSeams::default()
            },
        )
        .await
        .expect("working retry supervisor");
        let entry = sup
            .sessions
            .lock()
            .await
            .get(&id)
            .cloned()
            .expect("retained row reloads into memory");
        assert!(
            sup.teardown_session(&entry, &id, test_admission(&sup).await)
                .await
                .is_ok(),
            "a working scope manager must allow the retry"
        );
        assert!(
            sup.store
                .session(&id)
                .await
                .expect("read deleted row")
                .is_none(),
            "the successful retry must remove the row"
        );
    }

    /// Archive must not publish its archived flag after a failed scope kill;
    /// retaining an ordinary row is what makes the same request retryable.
    #[farhelm_testtrace::test]
    async fn archive_keeps_a_session_unarchived_when_scope_kill_fails_and_retries() {
        let id = uuid::Uuid::new_v4().to_string();
        let (state, sup, entry) = scoped_session(
            crate::scope::ScopeManager::fake_failing_kills(Arc::new(|_| {})),
            &id,
        )
        .await;
        let result = sup.teardown_for_archive(&entry, &id).await;
        assert!(matches!(result, Err(ArchiveError::Sweep(_))));
        assert!(
            !sup.store
                .session(&id)
                .await
                .expect("read retained archive row")
                .expect("row must remain")
                .archived,
            "archive refusal must not publish the archived flag"
        );
        assert!(
            sup.sessions.lock().await.contains_key(&id),
            "archive refusal must retain the in-memory entry"
        );
        drop(sup);

        let sup = Supervisor::new_with_seams(
            state.path(),
            dummy_exe(),
            SupervisorTimeouts::default(),
            SupervisorSeams {
                scopes: Arc::new(working_scopes()),
                ..SupervisorSeams::default()
            },
        )
        .await
        .expect("working retry supervisor");
        let entry = sup
            .sessions
            .lock()
            .await
            .get(&id)
            .cloned()
            .expect("retained row reloads into memory");
        assert!(
            sup.teardown_for_archive(&entry, &id).await.is_ok(),
            "a working scope manager must allow the archive retry"
        );
        assert!(
            sup.store
                .session(&id)
                .await
                .expect("read archived row")
                .expect("archive must retain the row")
                .archived,
            "the successful retry must publish the archived flag"
        );
    }

    /// Archiving publishes a new entry that SHARES the run's live cells —
    /// here the two hook-diagnostic flags — with the entry it replaced.
    ///
    /// The rule is the same one a rename follows and the opposite of the
    /// one a relaunch follows, which is precisely why it needs pinning:
    /// `relaunched_entry` mints `hooked` and `hook_warned` fresh on every
    /// generation, and somebody applying that reasoning here would break
    /// the tripwire. Archiving does not start a new launch. A tick already
    /// holding the pre-archive entry — the capture pass runs on its own
    /// schedule and resolves entries independently of archive — must be
    /// able to spend the tripwire's once-per-launch latch through the entry
    /// it has and have the published one see it, or a hooked session that
    /// is archived near its horizon warns twice.
    ///
    /// Asserted by pointer identity rather than by value, because value
    /// equality is exactly what a copied-flag implementation would also
    /// satisfy at the moment of the copy while still splitting the two
    /// writers afterwards.
    ///
    /// The entry is deliberately TERMINAL-LESS, which is the restart-gap
    /// shape archive already has to handle: it takes the tmux work out of
    /// the picture entirely, leaving the entry construction this test is
    /// about. The row still has to exist, because archive reads the durable
    /// tmux name before anything else.
    #[farhelm_testtrace::test]
    async fn archiving_shares_the_launchs_hook_cells_with_the_entry_it_replaces() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let id = "archived-session";
        sup.store
            .insert_session(
                StoredSession {
                    conversation_source: None,
                    id: id.to_string(),
                    parent: None,
                    archived: false,
                    title: "hooked".to_string(),
                    created_at: 1_700_000_000,
                    last_activity_at: 1_700_000_000,
                    last_work_started_at: 0,
                    creation_seq: 0,
                    cwd: "/tmp".to_string(),
                    invocation: "claude".to_string(),
                    launch: None,
                    tmux_name: format!("fh-{id}"),
                    pane: String::new(),
                    outcome: LastOutcome::Exited {
                        exit_code: Some(3),
                        annotation: None,
                    },
                    agent_kind: farhelm_proto::AgentKind::Claude,
                    // An integrated kind with the resume template its
                    // snapshot is required to carry: archive reads the row
                    // back through `SessionStore::session`, which refuses a
                    // hook-capable kind whose template could never be
                    // filled. A `Generic` row would sidestep that, but a
                    // hooked launch is by definition an integrated one, so
                    // the fixture stays the shape production produces.
                    resume_template: Some(vec![
                        "claude".to_string(),
                        "--resume".to_string(),
                        crate::agent_kind::CONVERSATION_PLACEHOLDER.to_string(),
                    ]),
                    canonical_cwd: Some("/tmp".to_string()),
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: None,
                    generation: 0,
                    launch_scoped: false,
                    source_profile: None,
                },
                None,
            )
            .await
            .expect("seed the session being archived");

        let mut entry = entry_with(
            None,
            LastOutcome::Exited {
                exit_code: Some(3),
                annotation: None,
            },
        );
        entry.info.id = id.to_string();
        let entry = Arc::new(entry);
        // The launch was hooked and the tripwire has not spoken yet: the
        // state in which BOTH flags still have work left to do.
        entry
            .hooked
            .store(true, std::sync::atomic::Ordering::Relaxed);

        // `ArchiveError` carries no `Debug`, so the failure is described
        // here rather than unwrapped.
        let Ok(archived) = sup.teardown_for_archive(&entry, id).await else {
            panic!("a terminal-less session archives without tmux");
        };

        assert_eq!(
            archived.outcome.lock().unwrap().clone(),
            LastOutcome::Exited {
                exit_code: Some(3),
                annotation: None,
            },
            "archiving an already-ended agent must retain its witnessed exit"
        );
        assert_eq!(
            archived.info.status,
            farhelm_proto::SessionStatus::Exited { exit_code: Some(3) }
        );
        assert_eq!(archived.info.annotation, None);

        assert!(
            Arc::ptr_eq(&entry.hooked, &archived.hooked),
            "the hook-injection flag must be the SAME cell across an archive"
        );
        assert!(
            Arc::ptr_eq(&entry.hook_warned, &archived.hook_warned),
            "the tripwire latch must be the SAME cell across an archive"
        );
        // And the sharing is live in the direction the bug takes: the
        // writer holds the pre-archive entry, the reader the published one.
        entry
            .hook_warned
            .store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(
            archived
                .hook_warned
                .load(std::sync::atomic::Ordering::Relaxed),
            "a tripwire warning spent through the pre-archive entry must not be spendable again"
        );
        assert!(
            archived.hooked.load(std::sync::atomic::Ordering::Relaxed),
            "and the archived entry must still describe the launch as hooked"
        );
    }

    /// A live owned pane is the one archive case that creates a new outcome:
    /// the teardown itself is the evidence for the user-stop annotation.
    /// This uses the supervisor's private tmux server and the ordinary entry
    /// fixture so the assertion covers the pane probe, process sweep, store
    /// write, and published in-memory entry together.
    #[farhelm_testtrace::test]
    async fn archiving_a_live_agent_records_the_stop_annotation() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let id = "live-archive";
        let tmux_name = format!("fh-{id}");
        let pane = sup
            .tmux
            .create_session(
                &tmux_name,
                "/tmp",
                80,
                24,
                &[],
                &["sleep".to_string(), "60".to_string()],
            )
            .await
            .expect("create the owned live pane");
        assert!(
            matches!(
                sup.tmux.pane_process(&tmux_name, &pane).await,
                Ok(PaneProbe::Owned(process)) if !process.dead
            ),
            "the fixture must prove the pane is live before archive relies on it"
        );
        sup.store
            .insert_session(
                StoredSession {
                    conversation_source: None,
                    id: id.to_string(),
                    parent: None,
                    archived: false,
                    title: id.to_string(),
                    created_at: 1_700_000_000,
                    last_activity_at: 1_700_000_000,
                    last_work_started_at: 0,
                    creation_seq: 0,
                    cwd: "/tmp".to_string(),
                    invocation: "sleep 60".to_string(),
                    launch: None,
                    tmux_name: tmux_name.clone(),
                    pane: pane.clone(),
                    outcome: LastOutcome::Running,
                    agent_kind: farhelm_proto::AgentKind::Generic,
                    resume_template: None,
                    canonical_cwd: Some("/tmp".to_string()),
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: None,
                    generation: 0,
                    launch_scoped: false,
                    source_profile: None,
                },
                None,
            )
            .await
            .expect("seed the live session row");
        let entry = Arc::new(entry_with(
            Some(super::super::terminals::Terminal { tmux_name, pane }),
            LastOutcome::Running,
        ));

        let Ok(archived) = sup.teardown_for_archive(&entry, id).await else {
            panic!("archive the live session");
        };

        assert_eq!(
            archived.outcome.lock().unwrap().clone(),
            LastOutcome::Exited {
                exit_code: None,
                annotation: Some(STOP_ANNOTATION.to_string()),
            }
        );
        assert_eq!(archived.info.annotation.as_deref(), Some(STOP_ANNOTATION));
        assert_eq!(
            archived.info.status,
            farhelm_proto::SessionStatus::Exited { exit_code: None }
        );
    }

    /// Delete must enumerate launch scopes from the manager, not only from
    /// the current row generation. A failed old-generation kill can leave a
    /// scrubbed daemon after the sweep reported clean, and deleting later is
    /// the last durable chance to name that cgroup.
    #[farhelm_testtrace::test]
    async fn deleting_enumerates_and_kills_a_previous_launch_generation_scope() {
        let state = StateDir::new();
        let id = uuid::Uuid::new_v4().to_string();
        let previous = crate::scope::unit_name(&id, 3).expect("a UUID names a launch scope");
        let observed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = {
            let observed = Arc::clone(&observed);
            Arc::new(move |op: &crate::scope::ScopeOp| {
                observed.lock().unwrap().push(op.clone());
            }) as crate::scope::ScopeOpSink
        };
        let seams = SupervisorSeams {
            scopes: Arc::new(
                crate::scope::ScopeManager::fake_with_matching_units_vanishing(
                    true,
                    vec![previous.clone()],
                    2,
                    sink,
                ),
            ),
            ..SupervisorSeams::default()
        };
        let sup = Supervisor::new_with_seams(
            state.path(),
            dummy_exe(),
            SupervisorTimeouts::default(),
            seams,
        )
        .await
        .expect("supervisor");
        sup.store
            .insert_session(
                StoredSession {
                    conversation_source: None,
                    id: id.clone(),
                    parent: None,
                    archived: false,
                    title: "previous scope".to_string(),
                    created_at: 1_700_000_000,
                    last_activity_at: 1_700_000_000,
                    last_work_started_at: 0,
                    creation_seq: 0,
                    cwd: "/tmp".to_string(),
                    invocation: "agent".to_string(),
                    launch: None,
                    tmux_name: format!("fh-{id}"),
                    pane: String::new(),
                    outcome: LastOutcome::Running,
                    agent_kind: farhelm_proto::AgentKind::Generic,
                    resume_template: None,
                    canonical_cwd: None,
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: None,
                    generation: 4,
                    launch_scoped: false,
                    source_profile: None,
                },
                None,
            )
            .await
            .expect("seed the session being deleted");
        let mut entry = entry_with(None, LastOutcome::Running);
        entry.info.id = id.clone();

        let Ok(()) = sup
            .teardown_session(&entry, &id, test_admission(&sup).await)
            .await
        else {
            panic!("a terminal-less session with a confirmed old scope deletes");
        };

        let observed = observed.lock().expect("scope operation sink poisoned");
        assert!(
            observed.iter().any(|op| matches!(
                op,
                crate::scope::ScopeOp::List(pattern)
                    if pattern == &crate::scope::launch_unit_glob(&id).unwrap()
            )),
            "delete must enumerate every launch generation: {observed:?}"
        );
        assert!(
            observed.iter().any(|op| matches!(
                op,
                crate::scope::ScopeOp::Kill { unit, .. } if unit == &previous
            )),
            "the previous launch scope returned by the glob must be killed: {observed:?}"
        );
    }

    /// A seeded terminal-less session row + in-memory entry with the given
    /// id and cwd — the E1 fixture's per-session scaffolding.
    async fn seeded_session(sup: &Arc<Supervisor>, id: &str, cwd: &str) -> Arc<SessionEntry> {
        sup.store
            .insert_session(
                StoredSession {
                    conversation_source: None,
                    id: id.to_string(),
                    parent: None,
                    archived: false,
                    title: id.to_string(),
                    created_at: 1_700_000_000,
                    last_activity_at: 1_700_000_000,
                    last_work_started_at: 0,
                    creation_seq: 0,
                    cwd: cwd.to_string(),
                    invocation: "agent".to_string(),
                    launch: None,
                    tmux_name: format!("fh-{id}"),
                    pane: String::new(),
                    outcome: LastOutcome::Running,
                    agent_kind: farhelm_proto::AgentKind::Generic,
                    resume_template: None,
                    canonical_cwd: Some(cwd.to_string()),
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: None,
                    generation: 0,
                    launch_scoped: true,
                    source_profile: None,
                },
                None,
            )
            .await
            .expect("seed the session row");
        let mut entry = entry_with(None, LastOutcome::Running);
        entry.info.id = id.to_string();
        entry.info.cwd = cwd.to_string();
        let entry = Arc::new(entry);
        sup.sessions
            .lock()
            .await
            .insert(id.to_string(), Arc::clone(&entry));
        entry
    }

    /// Durable Ready evidence and its flock inode let lifecycle tests observe
    /// exactly when final retirement grants cleanup authority. These fixtures
    /// have no running shim; the ordinary teardown still proves process absence.
    fn preparation_files(state_dir: &std::path::Path, checkout_id: &str) -> (PathBuf, PathBuf) {
        let path = state_dir
            .join("checkout-preparation")
            .join(format!("{checkout_id}.json"));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        crate::launch::write_preparation_state(
            &path,
            checkout_id,
            &crate::launch::PreparationState::Ready,
            &crate::files::RealFs,
        )
        .unwrap();
        let lock = crate::launch::preparation_lock_path(&path);
        std::fs::File::create(&lock).unwrap();
        assert!(path.is_file() && lock.is_file());
        (path, lock)
    }

    /// E1 (Design E): the last-reference delete archives the checkout —
    /// exact contents appear ONCE under farhelm-archived-working-copies,
    /// the source path is gone, the registry row retires — while a
    /// multi-member delete moves NOTHING, and an unmanaged directory is
    /// never touched.
    ///
    /// Preparation state and its lock survive Archive, owner deletion with a
    /// borrower, and a failed last Delete; successful final retirement removes
    /// both. This keeps cleanup tied to durable lifetime rather than visibility.
    ///
    /// The fixture plants registry rows through the SAME `&Connection`
    /// primitives production composes (`record_planned` + `allocate` +
    /// `add_member`), so `allocate`'s exclusive mkdir and identity capture
    /// are the real ones.
    #[farhelm_testtrace::test]
    async fn deleting_the_last_reference_archives_and_other_members_hold_the_directory() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let root = state.path().join("workroot");
        std::fs::create_dir_all(&root).expect("the checkout root");
        let team_id;
        {
            let conn = sup.store.conn.lock().expect("db mutex");
            let team = crate::working_copies::record_planned(
                &conn,
                &crate::working_copies::PlannedWorkingCopy {
                    id: uuid::Uuid::new_v4().to_string(),
                    canonical_root: root.to_string_lossy().into_owned(),
                    repo_owner: "octo".to_string(),
                    repo_name: "team".to_string(),
                    original_basename: "team".to_string(),
                    origin_session_id: "s-a".to_string(),
                    root_identity: None,
                    preparation_snapshot: None,
                },
            )
            .expect("record the team checkout plan");
            let team_dir = crate::working_copies::allocate(&conn, &team.id, None)
                .expect("allocate the team checkout");
            crate::working_copies::add_member(&conn, "s-b", &team.id)
                .expect("attach the second member");
            team_id = team.id;
            let nested = crate::working_copies::record_planned(
                &conn,
                &crate::working_copies::PlannedWorkingCopy {
                    id: uuid::Uuid::new_v4().to_string(),
                    canonical_root: root.to_string_lossy().into_owned(),
                    repo_owner: "octo".to_string(),
                    repo_name: "nested".to_string(),
                    original_basename: "nested".to_string(),
                    origin_session_id: "s-c".to_string(),
                    root_identity: None,
                    preparation_snapshot: None,
                },
            )
            .expect("record the nested checkout plan");
            crate::working_copies::allocate(&conn, &nested.id, None)
                .expect("allocate the nested checkout");
            std::fs::write(team_dir.canonical_path.join("file.txt"), "contents")
                .expect("seed the checkout's contents");
        }
        let team_path = root.join("team");
        let nested_path = root.join("nested");
        let unmanaged = root.join("unmanaged");
        std::fs::create_dir_all(&unmanaged).expect("the unmanaged directory");
        std::fs::write(unmanaged.join("keep.txt"), "keep").expect("seed the stranger");

        let entry_a = seeded_session(&sup, "s-a", &team_path.to_string_lossy()).await;
        let _entry_b = seeded_session(&sup, "s-b", &team_path.to_string_lossy()).await;
        let entry_c = seeded_session(&sup, "s-c", &nested_path.to_string_lossy()).await;
        let (preparation, preparation_lock) = preparation_files(state.path(), &team_id);
        let prepared_bytes = std::fs::read(&preparation).unwrap();
        let entry_a = sup
            .teardown_for_archive(&entry_a, "s-a")
            .await
            .unwrap_or_else(|_| panic!("Archive retains checkout preparation"));
        assert_eq!(std::fs::read(&preparation).unwrap(), prepared_bytes);
        assert!(preparation_lock.is_file());
        assert_eq!(
            sup.store.working_copy_member_count(&team_id).await.unwrap(),
            2
        );

        // Delete A: B still holds the checkout — no move, row gone, path
        // unchanged, membership down to one.
        sup.teardown_session(&entry_a, "s-a", test_admission(&sup).await)
            .await
            .unwrap_or_else(|_| panic!("delete the first member"));
        assert!(
            team_path.join("file.txt").exists(),
            "a delete that leaves another member must not move the checkout"
        );
        assert!(
            sup.store.session("s-a").await.expect("read").is_none(),
            "the first member's row is gone"
        );
        assert_eq!(
            sup.store
                .working_copy_member_count(&team_id)
                .await
                .expect("member count"),
            1,
            "the surviving member keeps its membership"
        );
        assert_eq!(std::fs::read(&preparation).unwrap(), prepared_bytes);
        assert!(preparation_lock.is_file());

        // Delete B: last reference — archive, source gone, retired record.
        let entry_b = sup
            .sessions
            .lock()
            .await
            .get("s-b")
            .cloned()
            .expect("the surviving member reloads");
        let archive_root = root.join(crate::working_copies::ARCHIVE_DIR_NAME);
        std::os::unix::fs::symlink(&unmanaged, &archive_root).unwrap();
        assert!(matches!(
            sup.teardown_session(&entry_b, "s-b", test_admission(&sup).await)
                .await,
            Err(TeardownError::FailClosed(_))
        ));
        assert!(team_path.join("file.txt").exists());
        assert!(sup.store.session("s-b").await.unwrap().is_some());
        assert_eq!(
            sup.store.working_copy_member_count(&team_id).await.unwrap(),
            1
        );
        assert_eq!(std::fs::read(&preparation).unwrap(), prepared_bytes);
        assert!(preparation_lock.is_file());
        std::fs::remove_file(&archive_root).unwrap();
        sup.teardown_session(&entry_b, "s-b", test_admission(&sup).await)
            .await
            .unwrap_or_else(|_| panic!("delete the last member"));
        assert!(!preparation.exists());
        assert!(!preparation_lock.exists());
        assert!(
            !team_path.exists(),
            "the last-reference delete vacates the source path"
        );
        let archive_root = root.join(crate::working_copies::ARCHIVE_DIR_NAME);
        let archived: Vec<_> = std::fs::read_dir(&archive_root)
            .expect("the archive directory")
            .collect();
        assert_eq!(
            archived.len(),
            1,
            "the checkout's contents appear exactly once in the archive"
        );
        assert!(
            archived[0]
                .as_ref()
                .unwrap()
                .path()
                .join("file.txt")
                .exists(),
            "the contents moved, not copied-and-dropped"
        );
        let team_row = sup
            .store
            .working_copy_rows()
            .await
            .expect("read the registry")
            .into_iter()
            .find(|row| row.id == team_id)
            .expect("the archived row survives as evidence history");
        assert_eq!(
            team_row.allocation_state,
            crate::working_copies::AllocationState::Retired,
            "the settled archive retires its record"
        );

        // Delete C: the second checkout archives the same way.
        sup.teardown_session(&entry_c, "s-c", test_admission(&sup).await)
            .await
            .unwrap_or_else(|_| panic!("delete the nested checkout's last member"));
        assert!(
            !nested_path.exists(),
            "the nested checkout's source is gone"
        );
        assert_eq!(
            std::fs::read_dir(&archive_root)
                .expect("the archive directory")
                .count(),
            2,
            "each checkout is archived exactly once"
        );

        // The unmanaged directory is untouched, always.
        assert!(
            unmanaged.join("keep.txt").exists(),
            "an unrelated unmanaged directory is never moved or removed"
        );
    }

    /// E2/E3 shared premise, fail-closed arm: a foreign object at a managed
    /// checkout's recorded source refuses the delete's move, retains the
    /// session row, and leaves the stranger untouched.
    #[farhelm_testtrace::test]
    async fn deleting_refuses_to_move_a_stranger_at_the_recorded_source() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let root = state.path().join("workroot");
        std::fs::create_dir_all(&root).expect("the checkout root");
        let checkout_id;
        {
            let conn = sup.store.conn.lock().expect("db mutex");
            let plan = crate::working_copies::record_planned(
                &conn,
                &crate::working_copies::PlannedWorkingCopy {
                    id: uuid::Uuid::new_v4().to_string(),
                    canonical_root: root.to_string_lossy().into_owned(),
                    repo_owner: "octo".to_string(),
                    repo_name: "swap".to_string(),
                    original_basename: "swap".to_string(),
                    origin_session_id: "s-swap".to_string(),
                    root_identity: None,
                    preparation_snapshot: None,
                },
            )
            .expect("record the checkout plan");
            let accepted = crate::working_copies::allocate(&conn, &plan.id, None)
                .expect("allocate the checkout");
            checkout_id = plan.id;
            // The stranger replaces the checkout after capture: identity
            // evidence now disagrees, and the delete must not move it. A
            // plain remove+recreate can be silently RE-IDENTIFIED by the
            // filesystem (freed inode numbers are reused immediately), so
            // the stranger is created only once its (dev,ino) provably
            // differs from the captured identity — retrying through filler
            // directories otherwise.
            std::fs::remove_dir_all(&accepted.canonical_path).expect("vacate the source");
            // Fillers are KEPT (removed only after the stranger differs):
            // removing them as we go would hand the freed low inode number
            // straight back to the next allocation and the loop would
            // never advance on a lowest-free allocator.
            let mut filler_root = accepted
                .canonical_path
                .parent()
                .unwrap()
                .join("stranger-fx");
            let mut made_fillers = 0usize;
            let mut differs = false;
            for _ in 0..64 {
                std::fs::create_dir_all(&accepted.canonical_path).expect("plant the stranger");
                let stranger =
                    std::fs::symlink_metadata(&accepted.canonical_path).expect("stat the stranger");
                use std::os::unix::fs::MetadataExt as _;
                if (stranger.dev(), stranger.ino()) != accepted.identity {
                    differs = true;
                    break;
                }
                std::fs::remove_dir_all(&accepted.canonical_path)
                    .expect("retry past an inode-number reuse");
                std::fs::create_dir_all(filler_root.join(format!("f{made_fillers}")))
                    .expect("consume an inode number");
                made_fillers += 1;
                filler_root = filler_root.join(format!("n{made_fillers}"));
            }
            std::fs::remove_dir_all(
                accepted
                    .canonical_path
                    .parent()
                    .unwrap()
                    .join("stranger-fx"),
            )
            .expect("drop the identity shifters");
            assert!(
                differs,
                "the fixture could not mint a stranger with a different identity"
            );
        }
        let entry = seeded_session(&sup, "s-swap", &root.join("swap").to_string_lossy()).await;
        let result = sup
            .teardown_session(&entry, "s-swap", test_admission(&sup).await)
            .await;
        assert!(
            matches!(result, Err(TeardownError::FailClosed(_))),
            "a foreign object at the recorded source fails closed"
        );
        assert!(
            sup.store.session("s-swap").await.expect("read").is_some(),
            "the failed delete retains the session row for a retry"
        );
        assert!(
            sup.store
                .working_copy_rows()
                .await
                .expect("read the registry")
                .into_iter()
                .any(|row| row.id == checkout_id),
            "the ownership evidence is preserved, never deleted over a stranger"
        );
        assert!(
            root.join("swap").exists(),
            "the stranger at the recorded path is untouched"
        );
    }

    /// Planned rows deliberately lack an accepted path. Explicit Delete must
    /// still name the candidate it leaves untouched, without adopting its inode
    /// or treating that diagnostic path as authority to archive unknown content.
    #[farhelm_testtrace::test]
    async fn deleting_an_unresolved_plan_names_and_preserves_the_unknown_path() {
        use std::os::unix::fs::MetadataExt;
        let capture = farhelm_testtrace::current_capture().expect("test capture");
        let state = StateDir::new();
        let root = state.path().join("workroot");
        let unknown = root.join("unknown");
        std::fs::create_dir_all(&unknown).unwrap();
        std::fs::write(unknown.join("payload"), b"not adopted").unwrap();
        let identity = std::fs::symlink_metadata(&unknown).unwrap();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .unwrap();
        let entry = seeded_session(&sup, "planned-origin", unknown.to_str().unwrap()).await;
        let checkout_id = uuid::Uuid::new_v4().to_string();
        {
            let conn = sup.store.conn.lock().unwrap();
            let plan = crate::working_copies::record_planned(
                &conn,
                &crate::working_copies::PlannedWorkingCopy {
                    id: checkout_id.clone(),
                    canonical_root: root.to_str().unwrap().into(),
                    repo_owner: "acme".into(),
                    repo_name: "unknown".into(),
                    original_basename: "unknown".into(),
                    origin_session_id: "planned-origin".into(),
                    root_identity: None,
                    preparation_snapshot: None,
                },
            )
            .unwrap();
            assert!(plan.canonical_path.is_none() && plan.path_identity.is_none());
            crate::working_copies::add_member(&conn, "planned-origin", &checkout_id).unwrap();
            conn.execute(
                "UPDATE sessions SET canonical_cwd = NULL WHERE id = 'planned-origin'",
                [],
            )
            .unwrap();
        }
        assert_eq!(
            sup.store
                .working_copy_member_count(&checkout_id)
                .await
                .unwrap(),
            1
        );
        sup.teardown_session(&entry, "planned-origin", test_admission(&sup).await)
            .await
            .unwrap_or_else(|_| panic!("explicit Delete retires only the unresolved metadata"));
        let warnings = capture
            .matching("delete retired an unresolved checkout plan")
            .unwrap();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].level, "WARN");
        assert_eq!(
            warnings[0].fields.get("path").map(String::as_str),
            unknown.to_str()
        );
        assert_eq!(
            warnings[0].fields.get("session").map(String::as_str),
            Some("planned-origin")
        );
        assert!(sup.store.session("planned-origin").await.unwrap().is_none());
        assert_eq!(
            sup.store
                .working_copy_member_count(&checkout_id)
                .await
                .unwrap(),
            0
        );
        assert!(sup.store.working_copy_rows().await.unwrap().is_empty());
        let after = std::fs::symlink_metadata(&unknown).unwrap();
        assert_eq!((after.dev(), after.ino()), (identity.dev(), identity.ino()));
        assert_eq!(
            std::fs::read(unknown.join("payload")).unwrap(),
            b"not adopted"
        );
        assert!(!root.join(crate::working_copies::ARCHIVE_DIR_NAME).exists());
    }

    /// Startup must refuse an otherwise valid pending archive when its path
    /// overlaps another active record. A separate nonoverlapping fixture proves
    /// that matching identities and the journal really permit ordinary recovery.
    #[farhelm_testtrace::test]
    async fn startup_archive_recovery_preserves_overlapping_registry_evidence() {
        use std::os::unix::fs::MetadataExt;
        for overlapping in [true, false] {
            let state = StateDir::new();
            let root = state.path().join("workroot");
            std::fs::create_dir(&root).unwrap();
            let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
                .await
                .unwrap();
            let mut allocated = Vec::new();
            for (id, basename, checkout_root) in [
                ("outer-origin", "outer", root.clone()),
                (
                    "inner-origin",
                    "inner",
                    if overlapping {
                        root.join("outer")
                    } else {
                        root.clone()
                    },
                ),
            ] {
                let accepted = {
                    let conn = sup.store.conn.lock().unwrap();
                    let plan = crate::working_copies::record_planned(
                        &conn,
                        &crate::working_copies::PlannedWorkingCopy {
                            id: uuid::Uuid::new_v4().to_string(),
                            canonical_root: checkout_root.to_str().unwrap().into(),
                            repo_owner: "acme".into(),
                            repo_name: basename.into(),
                            original_basename: basename.into(),
                            origin_session_id: id.into(),
                            root_identity: None,
                            preparation_snapshot: None,
                        },
                    )
                    .unwrap();
                    crate::working_copies::allocate(&conn, &plan.id, None).unwrap()
                };
                seeded_session(&sup, id, accepted.canonical_path.to_str().unwrap()).await;
                std::fs::write(accepted.canonical_path.join("payload"), basename).unwrap();
                allocated.push(accepted);
            }
            let outer = &allocated[0];
            let inner = &allocated[1];
            let archive_root = root.join(crate::working_copies::ARCHIVE_DIR_NAME);
            let destination = archive_root.join("outer-journaled");
            {
                let conn = sup.store.conn.lock().unwrap();
                assert_eq!(conn.execute(
                    "UPDATE working_copies SET allocation_state = 'archive_pending', archive_destination = 'outer-journaled' WHERE id = ?1",
                    [&outer.row.id],
                ).unwrap(), 1);
            }
            let before = sup.store.working_copy_rows().await.unwrap();
            assert_eq!(before.len(), 2);
            for accepted in &allocated {
                assert_eq!(
                    crate::working_copies::verify_identity(&accepted.row).unwrap(),
                    crate::working_copies::IdentityStatus::Matches
                );
                assert!(crate::working_copies::verified_root(&accepted.row).is_ok());
            }
            let memberships = (
                sup.store
                    .member_working_copies_all("outer-origin")
                    .await
                    .unwrap()
                    .len(),
                sup.store
                    .member_working_copies_all("inner-origin")
                    .await
                    .unwrap()
                    .len(),
            );
            assert!(!archive_root.exists());
            drop(sup);
            let reopened = Supervisor::new_with_exe(state.path(), dummy_exe())
                .await
                .unwrap();
            assert!(
                reopened.owns_state_dir(),
                "recovery must own the state directory"
            );
            let after = reopened.store.working_copy_rows().await.unwrap();
            assert_eq!(after.len(), before.len());
            for row in before {
                let retained = after
                    .iter()
                    .find(|candidate| candidate.id == row.id)
                    .unwrap();
                assert_eq!(retained.allocation_state, row.allocation_state);
                assert_eq!(retained.archive_destination, row.archive_destination);
                assert_eq!(retained.path_identity, row.path_identity);
                assert_eq!(retained.canonical_path, row.canonical_path);
            }
            assert_eq!(
                reopened
                    .store
                    .member_working_copies_all("outer-origin")
                    .await
                    .unwrap()
                    .len(),
                memberships.0
            );
            assert_eq!(
                reopened
                    .store
                    .member_working_copies_all("inner-origin")
                    .await
                    .unwrap()
                    .len(),
                memberships.1
            );
            assert!(
                reopened
                    .store
                    .session("outer-origin")
                    .await
                    .unwrap()
                    .is_some()
            );
            assert!(
                reopened
                    .store
                    .session("inner-origin")
                    .await
                    .unwrap()
                    .is_some()
            );
            let surviving_outer = if overlapping {
                assert!(
                    !archive_root.exists(),
                    "refusal precedes archive directory creation"
                );
                outer.canonical_path.clone()
            } else {
                assert!(!outer.canonical_path.exists());
                assert_eq!(std::fs::read_dir(&archive_root).unwrap().count(), 1);
                destination
            };
            let metadata = std::fs::metadata(&surviving_outer).unwrap();
            assert_eq!((metadata.dev(), metadata.ino()), outer.identity);
            assert_eq!(
                std::fs::read(surviving_outer.join("payload")).unwrap(),
                b"outer"
            );
            let metadata = std::fs::metadata(&inner.canonical_path).unwrap();
            assert_eq!((metadata.dev(), metadata.ino()), inner.identity);
            assert_eq!(
                std::fs::read(inner.canonical_path.join("payload")).unwrap(),
                b"inner"
            );
        }
    }

    /// Delete must retain ownership when a moved root hides the checkout at
    /// its recorded pathname. Reopening cannot turn that refusal into cleanup;
    /// genuine absence under the restored root still permits metadata deletion.
    /// Preparation artifacts follow that final metadata settlement as well.
    #[farhelm_testtrace::test]
    async fn deleting_a_missing_source_refuses_a_replaced_root_across_reopen() {
        use std::os::unix::fs::MetadataExt;
        for symlink_root in [false, true] {
            let state = StateDir::new();
            let root = state.path().join("workroot");
            let parked = state.path().join("parked");
            let foreign = state.path().join("foreign");
            std::fs::create_dir(&root).unwrap();
            let mut sup = Supervisor::new_with_exe(state.path(), dummy_exe())
                .await
                .unwrap();
            let checkout_id = uuid::Uuid::new_v4().to_string();
            let accepted = {
                let conn = sup.store.conn.lock().unwrap();
                crate::working_copies::record_planned(
                    &conn,
                    &crate::working_copies::PlannedWorkingCopy {
                        id: checkout_id.clone(),
                        canonical_root: root.to_str().unwrap().into(),
                        repo_owner: "acme".into(),
                        repo_name: "bar".into(),
                        original_basename: "bar".into(),
                        origin_session_id: "missing-origin".into(),
                        root_identity: None,
                        preparation_snapshot: None,
                    },
                )
                .unwrap();
                crate::working_copies::allocate(&conn, &checkout_id, None).unwrap()
            };
            seeded_session(&sup, "missing-origin", root.join("bar").to_str().unwrap()).await;
            let (preparation, preparation_lock) = preparation_files(state.path(), &checkout_id);
            std::fs::write(root.join("bar/payload"), b"owned bytes").unwrap();
            std::fs::rename(&root, &parked).unwrap();
            if symlink_root {
                std::fs::create_dir(&foreign).unwrap();
                std::os::unix::fs::symlink(&foreign, &root).unwrap();
            } else {
                std::fs::create_dir(&root).unwrap();
            }
            for reopen in [false, true] {
                if reopen {
                    drop(sup);
                    sup = Supervisor::new_with_exe(state.path(), dummy_exe())
                        .await
                        .unwrap();
                }
                assert!(!root.join("bar").exists());
                let metadata = std::fs::metadata(parked.join("bar")).unwrap();
                assert_eq!((metadata.dev(), metadata.ino()), accepted.identity);
                assert_eq!(
                    sup.store
                        .working_copy_member_count(&checkout_id)
                        .await
                        .unwrap(),
                    1
                );
                let entry = sup
                    .sessions
                    .lock()
                    .await
                    .get("missing-origin")
                    .cloned()
                    .unwrap();
                assert!(matches!(
                    sup.teardown_session(&entry, "missing-origin", test_admission(&sup).await)
                        .await,
                    Err(TeardownError::FailClosed(_))
                ));
                assert!(sup.store.session("missing-origin").await.unwrap().is_some());
                assert!(preparation.is_file() && preparation_lock.is_file());
                assert_eq!(
                    sup.store
                        .working_copy_member_count(&checkout_id)
                        .await
                        .unwrap(),
                    1
                );
                let row = sup
                    .store
                    .working_copy_rows()
                    .await
                    .unwrap()
                    .into_iter()
                    .find(|row| row.id == checkout_id)
                    .unwrap();
                assert_eq!(
                    row.allocation_state,
                    crate::working_copies::AllocationState::Allocated
                );
                assert_eq!(row.path_identity, Some(accepted.identity));
                assert_eq!(row.root_identity, accepted.row.root_identity);
                assert_eq!(row.archive_destination, None);
                assert_eq!(
                    std::fs::read(parked.join("bar/payload")).unwrap(),
                    b"owned bytes"
                );
                assert!(!root.join(crate::working_copies::ARCHIVE_DIR_NAME).exists());
                assert!(
                    !parked
                        .join(crate::working_copies::ARCHIVE_DIR_NAME)
                        .exists()
                );
            }
            if symlink_root {
                std::fs::remove_file(&root).unwrap();
            } else {
                std::fs::remove_dir(&root).unwrap();
            }
            std::fs::rename(&parked, &root).unwrap();
            std::fs::remove_file(root.join("bar/payload")).unwrap();
            std::fs::remove_dir(root.join("bar")).unwrap();
            let entry = sup
                .sessions
                .lock()
                .await
                .get("missing-origin")
                .cloned()
                .unwrap();
            sup.teardown_session(&entry, "missing-origin", test_admission(&sup).await)
                .await
                .unwrap_or_else(|_| {
                    panic!("matching root and missing checkout permit metadata cleanup")
                });
            assert!(sup.store.session("missing-origin").await.unwrap().is_none());
            assert!(!preparation.exists() && !preparation_lock.exists());
            assert_eq!(
                sup.store
                    .working_copy_member_count(&checkout_id)
                    .await
                    .unwrap(),
                0
            );
            assert!(
                !sup.store
                    .working_copy_rows()
                    .await
                    .unwrap()
                    .iter()
                    .any(|row| row.id == checkout_id)
            );
            assert!(!root.join(crate::working_copies::ARCHIVE_DIR_NAME).exists());
        }
    }

    /// R1.6/E2: either failed parent barrier keeps the session, membership
    /// and journal across repeated Delete and reopen. Once syncing succeeds,
    /// Delete retires the same moved inode without touching a replacement at
    /// the old source name. This tests the actual fsync seam, not a failure
    /// adjacent to the durability operation.
    #[farhelm_testtrace::test]
    async fn archive_parent_sync_failure_retains_evidence_until_retry_is_durable() {
        use std::os::unix::fs::MetadataExt;
        use std::sync::atomic::{AtomicBool, Ordering};

        for fail_archive_parent in [true, false] {
            let state = StateDir::new();
            let root = state.path().join("workroot");
            std::fs::create_dir(&root).unwrap();
            let source = root.join("checkout");
            let archive_root = root.join(crate::working_copies::ARCHIVE_DIR_NAME);
            std::fs::create_dir(&archive_root).unwrap();
            let fail_path = if fail_archive_parent {
                archive_root.clone()
            } else {
                root.clone()
            };
            let failing = Arc::new(AtomicBool::new(true));
            let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
            let sync: crate::working_copies::ArchiveParentSync = {
                let failing = failing.clone();
                let calls = calls.clone();
                Arc::new(move |path, file| {
                    let actual = file.metadata()?;
                    let expected = std::fs::metadata(path)?;
                    assert_eq!(
                        (actual.dev(), actual.ino()),
                        (expected.dev(), expected.ino())
                    );
                    calls.lock().unwrap().push(path.to_path_buf());
                    if path == fail_path && failing.load(Ordering::SeqCst) {
                        Err(std::io::Error::other(
                            "injected archive parent sync failure",
                        ))
                    } else {
                        file.sync_all()
                    }
                })
            };
            let mut sup = Supervisor::new_with_seams(
                state.path(),
                dummy_exe(),
                SupervisorTimeouts::default(),
                SupervisorSeams {
                    archive_parent_sync: Some(sync.clone()),
                    ..SupervisorSeams::default()
                },
            )
            .await
            .unwrap();
            let checkout_id = uuid::Uuid::new_v4().to_string();
            let owned_identity = {
                let conn = sup.store.conn.lock().unwrap();
                crate::working_copies::record_planned(
                    &conn,
                    &crate::working_copies::PlannedWorkingCopy {
                        id: checkout_id.clone(),
                        canonical_root: root.to_str().unwrap().into(),
                        repo_owner: "acme".into(),
                        repo_name: "checkout".into(),
                        original_basename: "checkout".into(),
                        origin_session_id: "sync-origin".into(),
                        root_identity: None,
                        preparation_snapshot: None,
                    },
                )
                .unwrap();
                crate::working_copies::allocate(&conn, &checkout_id, None)
                    .unwrap()
                    .identity
            };
            let mut entry = seeded_session(&sup, "sync-origin", source.to_str().unwrap()).await;
            std::fs::write(source.join("owned"), b"uncommitted content").unwrap();
            assert_eq!(
                sup.store
                    .working_copy_member_count(&checkout_id)
                    .await
                    .unwrap(),
                1
            );
            let mut destination = None;

            for attempt in 0..3 {
                calls.lock().unwrap().clear();
                let result = sup
                    .teardown_session(&entry, "sync-origin", test_admission(&sup).await)
                    .await;
                let message = match result {
                    Err(TeardownError::FailClosed(message)) => message,
                    _ => panic!("failed durability must refuse Delete"),
                };
                assert!(
                    message.contains("injected archive parent sync failure"),
                    "{message}"
                );
                assert!(
                    !message.contains("moved nothing"),
                    "rename has already succeeded"
                );
                let expected_calls = if fail_archive_parent {
                    vec![archive_root.clone()]
                } else {
                    vec![archive_root.clone(), root.clone()]
                };
                assert_eq!(*calls.lock().unwrap(), expected_calls);
                assert!(sup.store.session("sync-origin").await.unwrap().is_some());
                assert_eq!(
                    sup.store
                        .working_copy_member_count(&checkout_id)
                        .await
                        .unwrap(),
                    1
                );
                let row = sup
                    .store
                    .working_copy_rows()
                    .await
                    .unwrap()
                    .into_iter()
                    .find(|row| row.id == checkout_id)
                    .unwrap();
                assert_eq!(
                    row.allocation_state,
                    crate::working_copies::AllocationState::ArchivePending
                );
                let moved = archive_root.join(row.archive_destination.unwrap());
                let metadata = std::fs::metadata(&moved).unwrap();
                assert_eq!((metadata.dev(), metadata.ino()), owned_identity);
                assert_eq!(
                    std::fs::read(moved.join("owned")).unwrap(),
                    b"uncommitted content"
                );
                if let Some(previous) = &destination {
                    assert_eq!(
                        &moved, previous,
                        "retry must keep the already-moved destination"
                    );
                } else {
                    assert!(!source.exists(), "the failure is after the real rename");
                    destination = Some(moved);
                    // A foreign source makes a second rename observably wrong;
                    // recovery must rely on the matching archived identity.
                    std::fs::create_dir(&source).unwrap();
                    std::fs::write(source.join("foreign"), b"leave untouched").unwrap();
                    let foreign = std::fs::metadata(&source).unwrap();
                    assert_ne!((foreign.dev(), foreign.ino()), owned_identity);
                }
                assert_eq!(std::fs::read_dir(&archive_root).unwrap().count(), 1);
                if attempt == 1 {
                    drop(entry);
                    drop(sup);
                    calls.lock().unwrap().clear();
                    // The new owner attempts recovery during construction.
                    // Its failed barrier must leave the journal available
                    // for the explicit Delete retry below as well.
                    sup = Supervisor::new_with_seams(
                        state.path(),
                        dummy_exe(),
                        SupervisorTimeouts::default(),
                        SupervisorSeams {
                            archive_parent_sync: Some(sync.clone()),
                            ..SupervisorSeams::default()
                        },
                    )
                    .await
                    .unwrap();
                    assert_eq!(*calls.lock().unwrap(), expected_calls);
                    entry = sup
                        .sessions
                        .lock()
                        .await
                        .get("sync-origin")
                        .cloned()
                        .unwrap();
                }
            }

            let foreign = std::fs::metadata(&source).unwrap();
            failing.store(false, Ordering::SeqCst);
            calls.lock().unwrap().clear();
            sup.teardown_session(&entry, "sync-origin", test_admission(&sup).await)
                .await
                .unwrap_or_else(|_| panic!("both successful barriers must permit retirement"));
            assert_eq!(
                *calls.lock().unwrap(),
                vec![archive_root.clone(), root.clone()]
            );
            assert!(sup.store.session("sync-origin").await.unwrap().is_none());
            assert_eq!(
                sup.store
                    .working_copy_member_count(&checkout_id)
                    .await
                    .unwrap(),
                0
            );
            let row = sup
                .store
                .working_copy_rows()
                .await
                .unwrap()
                .into_iter()
                .find(|row| row.id == checkout_id)
                .unwrap();
            assert_eq!(
                row.allocation_state,
                crate::working_copies::AllocationState::Retired
            );
            let moved = destination.unwrap();
            assert_eq!(
                row.archive_destination.as_deref(),
                moved.file_name().unwrap().to_str()
            );
            let metadata = std::fs::metadata(&moved).unwrap();
            assert_eq!((metadata.dev(), metadata.ino()), owned_identity);
            assert_eq!(
                std::fs::read(moved.join("owned")).unwrap(),
                b"uncommitted content"
            );
            let after = std::fs::metadata(&source).unwrap();
            assert_eq!((after.dev(), after.ino()), (foreign.dev(), foreign.ino()));
            assert_eq!(
                std::fs::read(source.join("foreign")).unwrap(),
                b"leave untouched"
            );
            assert_eq!(std::fs::read_dir(&archive_root).unwrap().count(), 1);
        }
    }

    /// F10/R1.6: after rename but before metadata settlement, the journal's
    /// matching destination is authoritative even if someone reuses the old
    /// source name. Reopen and Delete must finish the original move without
    /// moving, deleting, or adopting that foreign directory.
    /// A post-commit preparation unlink failure preserves diagnostic evidence
    /// without undoing retirement or triggering another move after reopening.
    #[farhelm_testtrace::test]
    async fn delete_after_reopen_preserves_foreign_source_when_archive_destination_matches() {
        use std::os::unix::fs::MetadataExt;
        let state = StateDir::new();
        let root = state.path().join("workroot");
        std::fs::create_dir(&root).unwrap();
        let source = root.join("checkout");
        let archive_root = root.join(crate::working_copies::ARCHIVE_DIR_NAME);
        let destination = archive_root.join("checkout-journaled");
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .unwrap();
        let checkout_id = uuid::Uuid::new_v4().to_string();
        let owned_identity = {
            let conn = sup.store.conn.lock().unwrap();
            crate::working_copies::record_planned(
                &conn,
                &crate::working_copies::PlannedWorkingCopy {
                    id: checkout_id.clone(),
                    canonical_root: root.to_str().unwrap().into(),
                    repo_owner: "acme".into(),
                    repo_name: "bar".into(),
                    original_basename: "checkout".into(),
                    origin_session_id: "archive-origin".into(),
                    root_identity: None,
                    preparation_snapshot: None,
                },
            )
            .unwrap();
            crate::working_copies::allocate(&conn, &checkout_id, None)
                .unwrap()
                .identity
        };
        let entry = seeded_session(&sup, "archive-origin", source.to_str().unwrap()).await;
        let (preparation, preparation_lock) = preparation_files(state.path(), &checkout_id);
        let prepared_bytes = std::fs::read(&preparation).unwrap();
        // A directory at the lock pathname causes a real unlink failure,
        // independent of uid. The completed archive must still retire once.
        std::fs::remove_file(&preparation_lock).unwrap();
        std::fs::create_dir(&preparation_lock).unwrap();
        std::fs::write(source.join("owned"), b"original checkout").unwrap();
        std::fs::create_dir(&archive_root).unwrap();
        {
            let conn = sup.store.conn.lock().unwrap();
            assert_eq!(conn.execute(
                "UPDATE working_copies SET allocation_state = 'archive_pending', archive_destination = ?2 WHERE id = ?1",
                rusqlite::params![checkout_id, "checkout-journaled"],
            ).unwrap(), 1);
        }
        // Model the precise crash boundary: journal committed and filesystem
        // rename completed, while the session and its membership remain.
        std::fs::rename(&source, &destination).unwrap();
        std::fs::create_dir(&source).unwrap();
        std::fs::write(source.join("foreign"), b"leave me here").unwrap();
        let foreign = std::fs::metadata(&source).unwrap();
        let archived = std::fs::metadata(&destination).unwrap();
        assert_eq!((archived.dev(), archived.ino()), owned_identity);
        assert_ne!((foreign.dev(), foreign.ino()), owned_identity);
        assert_eq!(
            sup.store
                .working_copy_member_count(&checkout_id)
                .await
                .unwrap(),
            1
        );
        drop(entry);
        drop(sup);

        let reopened = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .unwrap();
        let row = reopened
            .store
            .working_copy_rows()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == checkout_id)
            .unwrap();
        assert_eq!(
            row.allocation_state,
            crate::working_copies::AllocationState::ArchivePending
        );
        let entry = reopened
            .sessions
            .lock()
            .await
            .get("archive-origin")
            .cloned()
            .unwrap();
        reopened
            .teardown_session(&entry, "archive-origin", test_admission(&reopened).await)
            .await
            .unwrap_or_else(|_| panic!("Delete must settle the matching archived identity"));
        assert!(
            reopened
                .store
                .session("archive-origin")
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            reopened
                .store
                .working_copy_member_count(&checkout_id)
                .await
                .unwrap(),
            0
        );
        let row = reopened
            .store
            .working_copy_rows()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == checkout_id)
            .unwrap();
        assert_eq!(
            row.allocation_state,
            crate::working_copies::AllocationState::Retired
        );
        let after = std::fs::metadata(&source).unwrap();
        assert_eq!((after.dev(), after.ino()), (foreign.dev(), foreign.ino()));
        assert_eq!(
            std::fs::read(source.join("foreign")).unwrap(),
            b"leave me here"
        );
        assert_eq!(
            std::fs::read(destination.join("owned")).unwrap(),
            b"original checkout"
        );
        assert_eq!(std::fs::read_dir(&archive_root).unwrap().count(), 1);
        assert_eq!(std::fs::read(&preparation).unwrap(), prepared_bytes);
        assert!(
            preparation_lock.is_dir(),
            "failed cleanup preserves the offending artifact"
        );
        let warnings = farhelm_testtrace::current_capture()
            .unwrap()
            .matching("checkout retirement committed but preparation cleanup failed")
            .unwrap();
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].fields.get("path").map(String::as_str),
            preparation_lock.to_str()
        );
        assert_eq!(warnings[0].fields.get("working_copy"), Some(&checkout_id));
        drop(entry);
        drop(reopened);
        let reopened = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .unwrap();
        assert!(
            reopened
                .store
                .session("archive-origin")
                .await
                .unwrap()
                .is_none()
        );
        let metadata = std::fs::metadata(&destination).unwrap();
        assert_eq!((metadata.dev(), metadata.ino()), owned_identity);
        assert_eq!(std::fs::read_dir(&archive_root).unwrap().count(), 1);
        assert_eq!(std::fs::read(&preparation).unwrap(), prepared_bytes);
        assert!(preparation_lock.is_dir());
    }

    /// A real Existing create queued behind last-reference Delete must
    /// revalidate the now-absent cwd and insert no session or membership.
    #[farhelm_testtrace::test]
    async fn existing_create_after_last_reference_delete_refuses_the_archived_path() {
        existing_create_races_last_reference_delete(false).await;
    }

    /// A real Existing create admitted first must commit its reference before
    /// Delete counts members, preserving the directory for the new borrower.
    #[farhelm_testtrace::test]
    async fn existing_create_before_last_reference_delete_preserves_the_checkout() {
        existing_create_races_last_reference_delete(true).await;
    }

    /// Exercise both admission orders with the production create and teardown
    /// paths. The chosen winner holds admission before either path starts;
    /// actual Pending acquisition proves contention. Both paths then run to
    /// completion, and durable membership plus sentinel location distinguish
    /// an admitted borrower from a mere mutex handoff.
    async fn existing_create_races_last_reference_delete(create_first: bool) {
        let state = StateDir::new();
        let waiting = Arc::new(tokio::sync::Notify::new());
        let signal = waiting.clone();
        let sup = Supervisor::new_with_seams(
            state.path(),
            dummy_exe(),
            SupervisorTimeouts::default(),
            SupervisorSeams {
                create_directory_waiting: Some(Arc::new(move || signal.notify_one())),
                ..SupervisorSeams::default()
            },
        )
        .await
        .expect("supervisor");
        let root = state.path().join("workroot");
        std::fs::create_dir_all(&root).expect("the checkout root");
        let working_copy_id;
        {
            let conn = sup.store.conn.lock().expect("db mutex");
            let plan = crate::working_copies::record_planned(
                &conn,
                &crate::working_copies::PlannedWorkingCopy {
                    id: uuid::Uuid::new_v4().to_string(),
                    canonical_root: root.to_string_lossy().into_owned(),
                    repo_owner: "octo".to_string(),
                    repo_name: "race".to_string(),
                    original_basename: "race".to_string(),
                    origin_session_id: "s-race".to_string(),
                    root_identity: None,
                    preparation_snapshot: None,
                },
            )
            .expect("record the checkout plan");
            crate::working_copies::allocate(&conn, &plan.id, None).expect("allocate the checkout");
            working_copy_id = plan.id;
        }
        let source = root.join("race");
        let cwd = source.to_str().unwrap();
        std::fs::write(source.join("sentinel"), b"keep").unwrap();
        let entry = seeded_session(&sup, "s-race", cwd).await;
        assert_eq!(
            sup.store
                .working_copy_member_count(&working_copy_id)
                .await
                .unwrap(),
            1
        );
        assert!(sup.store.session("s-race").await.unwrap().is_some());
        assert!(source.is_dir());
        let inputs = CreateInputs {
            cwd,
            parent: None,
            github_checkout: None,
            mode: CreateMode::Raw {
                invocation: "agent".into(),
                agent_kind: None,
                resume_template: None,
                source_profile: None,
                launch: None,
            },
            title: None,
            cols: 80,
            rows: 24,
        };
        if create_first {
            let guards = sup.admit_create(None, None).await.unwrap();
            let admission = Arc::clone(&sup.working_copy_operations).lock_owned();
            tokio::pin!(admission);
            std::future::poll_fn(|cx| {
                assert!(
                    std::future::Future::poll(admission.as_mut(), cx).is_pending(),
                    "Delete must queue behind the admitted create"
                );
                std::task::Poll::Ready(())
            })
            .await;
            let (created, deleted) =
                tokio::time::timeout(std::time::Duration::from_secs(10), async {
                    tokio::join!(sup.create_session_admitted(inputs, None, guards), async {
                        sup.teardown_session(&entry, "s-race", admission.await)
                            .await
                    })
                })
                .await
                .expect("create and Delete complete");
            let created = created.expect("admitted Existing create succeeds");
            deleted.unwrap_or_else(|_| panic!("Delete succeeds with a borrower"));
            assert_eq!(
                sup.store
                    .working_copy_member_count(&working_copy_id)
                    .await
                    .unwrap(),
                1
            );
            assert!(sup.store.session(&created.id).await.unwrap().is_some());
            assert_eq!(std::fs::read(source.join("sentinel")).unwrap(), b"keep");
            assert!(!root.join(crate::working_copies::ARCHIVE_DIR_NAME).exists());
        } else {
            let guard = test_admission(&sup).await;
            let delete = sup.teardown_session(&entry, "s-race", guard);
            let create = sup.create_session(inputs, None);
            tokio::pin!(create);
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                tokio::select! {
                    _ = &mut create => panic!("create must wait behind Delete"),
                    _ = waiting.notified() => {},
                }
            })
            .await
            .expect("create observes Pending directory admission");
            let (deleted, refused) =
                tokio::time::timeout(std::time::Duration::from_secs(10), async {
                    tokio::join!(delete, &mut create)
                })
                .await
                .expect("Delete and create complete");
            deleted.unwrap_or_else(|_| panic!("last-reference Delete succeeds"));
            let refused = refused.expect_err("create revalidates the archived cwd");
            assert!(
                format!("{refused:#}").contains("working directory"),
                "{refused:#}"
            );
            assert!(!source.exists());
            assert_eq!(
                sup.store
                    .working_copy_member_count(&working_copy_id)
                    .await
                    .unwrap(),
                0
            );
            assert!(sup.store.load_all().await.unwrap().is_empty());
            let archives: Vec<_> =
                std::fs::read_dir(root.join(crate::working_copies::ARCHIVE_DIR_NAME))
                    .unwrap()
                    .map(|entry| entry.unwrap().path())
                    .collect();
            assert_eq!(archives.len(), 1);
            assert_eq!(
                std::fs::read(archives[0].join("sentinel")).unwrap(),
                b"keep"
            );
        }
        assert!(sup.store.session("s-race").await.unwrap().is_none());
    }
}
