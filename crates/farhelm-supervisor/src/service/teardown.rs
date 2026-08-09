//! Whole-session teardown: everything `DeleteSession` does to make a
//! session stop existing, with nothing about how the request is answered.
//!
//! Delete is the slowest and least reversible thing this supervisor does,
//! and its steps are ordered against each other for reasons that are not
//! recoverable from reading any one of them: uploads are cancelled before
//! the multi-second process sweep so nothing goes on writing into a
//! directory that is about to vanish; tab scope units are enumerated from
//! two independent sources because a tmux server that died first leaves no
//! windows to read tab ids from while a scrubbed tab daemon keeps running
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
//! tab, removes terminal-only launch artifacts and snapshots, and detaches
//! every viewer. It then KEEPS the database row and committed attachment
//! directory, marks the row archived, and records the deliberate teardown
//! as `Exited` with `STOP_ANNOTATION`. That separate path is intentional:
//! treating archive as a delete mode would make the files it must preserve
//! depend on branches inside delete's fail-closed removal sequence.

use super::connection::notify_detached;
use super::core::{ArchiveStage, SessionEntry, Supervisor};
use super::launch_artifacts::{remove_fail_closed, remove_launch_artifacts_for_session};
use super::snapshots::snapshot_path;
use super::sweep::{SweepTarget, reap_process_tree};
use super::terminals::{ActiveAttach, AttachmentKey};
use super::ticker::ActivitySample;
use super::uploads::abort_session_uploads;
use crate::store::LastOutcome;
use farhelm_proto::{STOP_ANNOTATION, SessionStatus};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::debug;

/// Every way a teardown can fail before the session is gone, as one
/// variant per failure CLASS — not per failure site. The four probe/sweep
/// variants each have exactly one site; [`TeardownError::FailClosed`]
/// covers the five steps inside the fail-closed block, which is sound
/// because those five already render their own site-specific message and
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
    /// tmux could not be asked what process the agent's pane holds, so the
    /// sweep has no root pid to work from.
    PaneProbe(anyhow::Error),
    /// tmux could not be asked for this session's tabs. Strictly a
    /// failure: see the call site for why "we could not ask" must not
    /// collapse into "there are none".
    TabRediscovery(anyhow::Error),
    /// A systemd user manager exists but would not enumerate this
    /// session's tab scopes.
    TabScopeEnumeration(anyhow::Error),
    /// The process-tree sweep itself failed — not "nothing was found to
    /// kill", but "this could not be confirmed".
    Sweep(anyhow::Error),
    /// The fail-closed block failed — the tmux kill, one of the launch-
    /// artifact/snapshot/attachment removals, or the row deletion and
    /// reservation settlement.
    ///
    /// The one variant that does NOT carry a source error: those five
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
pub(crate) enum ArchiveError {
    PaneProbe(anyhow::Error),
    TabRediscovery(anyhow::Error),
    TabScopeEnumeration(anyhow::Error),
    Sweep(anyhow::Error),
    FailClosed(String),
}

impl Supervisor {
    /// Shut down an entire session while preserving its durable metadata
    /// and committed attachments.
    ///
    /// The caller holds the session lifecycle claim across this function.
    /// The archive flag is committed only after every process, tab, tmux
    /// terminal, and terminal-only artifact is gone; a failure therefore
    /// leaves an ordinary visible session that can be retried. Committed
    /// attachment files are never moved or removed.
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
                self.tmux
                    .pane_process(&terminal.tmux_name, &terminal.pane)
                    .await
                    .map_err(ArchiveError::PaneProbe)?
            }
            None => None,
        };
        let root_pid = live_pane.filter(|pane| !pane.dead).map(|pane| pane.pid);

        // Archive reaches the same whole-session ownership boundary as
        // delete: tabs carry separate cgroup units, and the manager is the
        // only source left when tmux died before its scrubbed daemon did.
        let mut units = entry.scope.clone().into_iter().collect::<Vec<_>>();
        if let Some(terminal) = entry.terminal.as_ref() {
            if let Some(gate) = &self.seams.archive_gate {
                gate(ArchiveStage::TabRediscovery)
                    .await
                    .map_err(ArchiveError::TabRediscovery)?;
            }
            let tabs = self
                .session_tabs(terminal)
                .await
                .map_err(ArchiveError::TabRediscovery)?;
            units.extend(
                tabs.iter()
                    .filter_map(|tab| crate::scope::tab_unit_name(session_id, &tab.id)),
            );
        }
        if let Some(glob) = crate::scope::tab_unit_glob(session_id) {
            if let Some(gate) = &self.seams.archive_gate {
                gate(ArchiveStage::ScopeEnumeration)
                    .await
                    .map_err(ArchiveError::TabScopeEnumeration)?;
            }
            match self.seams.scopes.units_matching(&glob).await {
                Ok(found) => units.extend(found),
                Err(error) if !self.seams.scopes.available().await => debug!(
                    session = %session_id,
                    error = %format!("{error:#}"),
                    "no systemd user manager to enumerate this archived session's tab scopes; \
                     the process-tree sweep is the whole mechanism"
                ),
                Err(error) => return Err(ArchiveError::TabScopeEnumeration(error)),
            }
        }
        units.sort();
        units.dedup();
        if let Some(gate) = &self.seams.archive_gate {
            gate(ArchiveStage::Sweep)
                .await
                .map_err(ArchiveError::Sweep)?;
        }
        // Everything above is a read-only preflight. Keep transfers alive
        // until those checks have proved teardown can start: a refused
        // archive must not discard an upload and then claim nothing changed.
        // Once the checks pass, cancelling and joining immediately before
        // the first process kill closes the other side of the contract — no
        // transfer can publish into a session while it is being made
        // terminal-less. Committed attachments remain untouched.
        abort_session_uploads(self, session_id, "the session was archived", false).await;
        reap_process_tree(
            &self.seams.scopes,
            &units,
            root_pid,
            session_id,
            &SweepTarget::WholeSession,
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
            // Launch specs can contain credentials and the snapshot is
            // terminal content. Neither is metadata an archive promises to
            // retain, and keeping either would contradict the explicit
            // "terminal contents are gone" contract.
            remove_launch_artifacts_for_session(&self.state_dir, session_id).await?;
            remove_fail_closed(
                &snapshot_path(&self.state_dir, session_id),
                "alt-screen snapshot",
            )
            .await?;
            self.store
                .archive_session(session_id)
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

        let outcome = LastOutcome::Exited {
            exit_code: None,
            annotation: Some(STOP_ANNOTATION.to_string()),
        };
        let mut info = entry.info.clone();
        info.archived = true;
        info.status = SessionStatus::Exited { exit_code: None };
        info.annotation = Some(STOP_ANNOTATION.to_string());
        info.tabs.clear();
        let archived = Arc::new(SessionEntry {
            info,
            terminal: None,
            outcome: Arc::new(std::sync::Mutex::new(outcome)),
            snapshot: entry.snapshot.clone(),
            canonical_cwd: entry.canonical_cwd.clone(),
            first_input: Arc::clone(&entry.first_input),
            capture: Arc::clone(&entry.capture),
            activity: ActivitySample::unsampled(),
            generation: entry.generation,
            // Keep the prior launch's scope identity so a restart can run
            // its ordinary leftover sweep. Archive has already emptied it,
            // but losing the identity would weaken that defense after a
            // partial external cleanup.
            scope: entry.scope.clone(),
        });
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
    ) -> Result<(), TeardownError> {
        // In-flight uploads are cancelled FIRST — before the
        // process sweep, not after it. The sweep is where a delete
        // spends its seconds (a grace period plus several /proc
        // walks), and a transfer left running through it goes on
        // writing into the directory this delete is about to take
        // away, for as long as the sweep lasts. Cancelling first
        // costs nothing (the transfer is doomed either way) and
        // bounds those writes to the instant it takes each task to
        // notice.
        //
        // `abort_session_uploads` also WAITS for each task to
        // finish cleaning up, so from here on nothing can write
        // into (or publish into) that directory, and the lifecycle
        // claim the caller has held since its first line keeps a
        // new transfer from staging into it (see `stage_upload`).
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
            Some(terminal) => self
                .tmux
                .pane_process(&terminal.tmux_name, &terminal.pane)
                .await
                .map_err(TeardownError::PaneProbe)?,
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
        // for every unit matching this session's tab glob, which
        // needs no tmux at all. A failure to ENUMERATE fails the
        // delete outright, row retained: publishing "deleted" over
        // an unenumerated cgroup is exactly the unreapable,
        // invisible agent lore/2026-07-27-m2-process-tree-stop.md
        // ends on.
        let mut units = entry.scope.clone().into_iter().collect::<Vec<_>>();
        if let Some(terminal) = entry.terminal.as_ref() {
            // Strict: "we could not ask tmux" is not "there
            // are no tabs", and a delete that assumed the
            // latter would skip live tab scopes.
            let tabs = self
                .session_tabs(terminal)
                .await
                .map_err(TeardownError::TabRediscovery)?;
            units.extend(
                tabs.iter()
                    .filter_map(|tab| crate::scope::tab_unit_name(session_id, &tab.id)),
            );
        }
        if let Some(glob) = crate::scope::tab_unit_glob(session_id) {
            match self.seams.scopes.units_matching(&glob).await {
                Ok(found) => units.extend(found),
                // Only a host with a usable manager can be asked at
                // all; where there is none this is not a failure,
                // it is the sweep-only world M2 already lived in.
                Err(e) if !self.seams.scopes.available().await => debug!(
                    session = %session_id, error = %format!("{e:#}"),
                    "no systemd user manager to enumerate this session's tab scopes;                              the process-tree sweep is the whole mechanism"
                ),
                Err(e) => return Err(TeardownError::TabScopeEnumeration(e)),
            }
        }
        units.sort();
        units.dedup();
        reap_process_tree(
            &self.seams.scopes,
            &units,
            root_pid,
            session_id,
            &SweepTarget::WholeSession,
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
        let teardown: Result<(), String> = async {
            if let Some(error) = forwarder_error {
                return Err(error);
            }
            if let Some(terminal) = entry.terminal.as_ref() {
                self.tmux
                    .kill_session(&terminal.tmux_name)
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
            // Same fail-closed treatment as the launch artifacts
            // above and for the same reason: the snapshot can hold
            // secrets an agent echoed to an alt-screen app, and
            // delete is the last moment anything will ever come
            // back to remove it.
            remove_fail_closed(
                &snapshot_path(&self.state_dir, session_id),
                "alt-screen snapshot",
            )
            .await?;
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
            // Settles this session's create reservations in the
            // same transaction as the row removal, which is what
            // turns them into TOMBSTONES rather than stale claims:
            // a replay of one of those intent keys must report the
            // gone-error, never a dead id and never a fresh
            // duplicate (PLAN_M3.md item 6; the store method's own
            // docs carry the argument).
            self.store
                .delete_session_settling_reservations(session_id)
                .await
                .map_err(|e| format!("{e:#}"))
        }
        .await;

        if let Err(err_msg) = teardown {
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
        self.sessions.lock().await.remove(session_id);
        // The row is gone, so the quarantined attachments now
        // belong to nothing and can be removed for real. Failure
        // here is logged rather than reported: there is no row
        // left to retain for a retry, the caller's delete
        // genuinely succeeded, and the next startup reconciles
        // whatever is left.
        if let Some(parked) = quarantined {
            crate::attachments::discard_quarantined(&parked).await;
        }

        for (channel, notify) in &notify_detach {
            notify_detached(notify, *channel, "session deleted".to_string());
        }
        drop(attachments);
        Ok(())
    }
}
