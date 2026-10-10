//! Whole-session teardown: everything `DeleteSession` does to make a
//! session stop existing, with nothing about how the request is answered.
//!
//! Delete is the slowest and least reversible thing this supervisor does,
//! and its steps are ordered against each other for reasons that are not
//! recoverable from reading any one of them: refusal-prone read-only
//! preflights happen before uploads are cancelled; cancellation still comes
//! before the multi-second process sweep so nothing goes on writing into a
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
use super::connection::notify_detached;
use super::core::{SessionEntry, Supervisor, unknown_pane_owner_refusal};
use super::launch_artifacts::remove_launch_artifacts_for_session;
use super::sweep::{
    ScopeKillFailure, ScopeUnits, SweepTarget, capture_process_identity, reap_process_tree,
};
use super::terminals::{ActiveAttach, AttachmentKey, SinkReapWait};
use super::uploads::abort_session_uploads;
use crate::tmux::PaneProbe;

use std::path::PathBuf;
use tracing::{debug, error, warn};

/// Every way deletion can fail before the session is gone.
///
/// Each variant is fail-closed: the durable row remains available for a
/// later retry whenever teardown cannot prove the process tree and terminal
/// are gone.
pub(crate) enum TeardownError {
    PaneProbe(anyhow::Error),
    TabRediscovery(anyhow::Error),
    TabScopeEnumeration(anyhow::Error),
    Sweep(anyhow::Error),
    FailClosed(String),
}

/// Why a Delete left its checkout unarchived, as the notice will tell it.
///
/// `may_have_moved` separates the two outcomes a user must not confuse:
/// `None` means nothing was moved and the folder is still at its recorded
/// path; `Some` means a rename was attempted or journaled and its outcome is
/// unknown, so the folder may be at the recorded path or at the archive
/// destination (named when known).
struct ArchiveSkipped {
    reason: String,
    may_have_moved: Option<Option<String>>,
}

/// Whether an archive error is the filesystem refusing the rename outright
/// (see `working_copies::WorkingCopyError::RenameRefused`), which moved
/// nothing, as opposed to a failure whose outcome is unknown.
fn rename_refused(error: &anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<crate::working_copies::WorkingCopyError>(),
        Some(crate::working_copies::WorkingCopyError::RenameRefused { .. })
    )
}

/// The full path of a journaled archive destination, which the registry
/// stores as a bare name inside the root's archive folder.
fn journaled_destination_path(row: &crate::working_copies::WorkingCopyRow, name: &str) -> String {
    PathBuf::from(&row.canonical_root)
        .join(crate::working_copies::ARCHIVE_DIR_NAME)
        .join(name)
        .display()
        .to_string()
}

impl ArchiveSkipped {
    /// A refusal made before any move, so the folder has not been touched.
    fn untouched(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
            may_have_moved: None,
        }
    }
}

impl Supervisor {
    /// The live panes, other than the recorded agent pane, that Delete roots
    /// its process walk at: every pane under the session's tmux name, which
    /// is its tabs in ordinary use.
    ///
    /// The environment-marker scan finds a tab's processes only through a
    /// marked process to expand from, normally the tab's own shell. On macOS
    /// the kernel withholds the environment of platform binaries
    /// (`/usr/bin/env`, the login shell, `ssh`, `caffeinate`) even from the
    /// same user, so the shell reads as unmarked, and a pane process that
    /// replaced itself with a scrubbed environment has the same effect
    /// anywhere. On a host without a usable systemd user manager there is no
    /// tab scope to fall back on, so a PPID walk from the tab pane is the one
    /// thing that reaches the tab's processes, as Close Tab already does for
    /// a single tab. What a walk cannot reach (a descendant that both
    /// reparented away and lost its marker) stays the accepted manager-less
    /// residual of `kill_process_tree`.
    ///
    /// A session with no recorded terminal (a restart gap, or a reload that
    /// has not seen its pane yet) contributes every pane found under its
    /// durable tmux name, agent pane included, since none is recorded to
    /// root at. A pane that disappears or changes hands between the listing
    /// and its probe is skipped; failing to ask tmux or to read a live
    /// pane's process fails the delete, row retained, like the agent pane's
    /// own probe.
    async fn other_pane_roots(
        &self,
        entry: &SessionEntry,
        session_id: &str,
    ) -> Result<Vec<(u32, u64)>, TeardownError> {
        let (tmux_name, agent_pane) = match entry.terminal.as_ref() {
            Some(terminal) => (terminal.tmux_name.clone(), Some(terminal.pane.as_str())),
            None => match self
                .store
                .tmux_name(session_id)
                .await
                .map_err(TeardownError::TabRediscovery)?
            {
                Some(tmux_name) => (tmux_name, None),
                None => return Ok(Vec::new()),
            },
        };
        let states = self
            .tmux
            .pane_states()
            .await
            .map_err(TeardownError::TabRediscovery)?;
        let mut roots = Vec::new();
        for (pane, state) in &states {
            if state.session_name != tmux_name || state.dead || Some(pane.as_str()) == agent_pane {
                continue;
            }
            if let PaneProbe::Owned(process) = self
                .tmux
                .pane_process(&tmux_name, pane)
                .await
                .map_err(TeardownError::PaneProbe)?
                && !process.dead
            {
                roots.extend(capture_process_identity(process.pid).map_err(TeardownError::Sweep)?);
            }
        }
        Ok(roots)
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
    /// rather than acquired here. The lock order (the table on
    /// `Supervisor`) is directory admission BEFORE the lifecycle claim
    /// (R1.1), so this function must NEVER
    /// acquire the mutex itself — it would order lifecycle → directory
    /// against every create's intent → directory → lifecycle sequence and
    /// form a cycle with a create waiting on this very delete.
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
    ) -> Result<Option<String>, TeardownError> {
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
        // Capture the pane's identity before tab discovery, scope
        // enumeration, and archive work can give the kernel time to recycle
        // this pid. A vanished process leaves no root; an unreadable one
        // fails the delete rather than sweeping without it.
        let root_identity = match live_pane.filter(|pane| !pane.dead) {
            Some(pane) => capture_process_identity(pane.pid).map_err(TeardownError::Sweep)?,
            None => None,
        };
        // Every other live pane of the session is a root too, captured at
        // the same boundary for the same recycling reason.
        let other_roots = self.other_pane_roots(entry, session_id).await?;
        // `WholeSession`: delete is the one lifecycle operation
        // that takes tabs down with the agent (SPEC.md — stop
        // leaves them running), so this
        // sweep deliberately does NOT subtract tab processes. It
        // roots its walk at every tab pane as well as the agent's
        // (`other_pane_roots`): the marker scan reaches a tab's
        // processes only by expanding from a marked shell, macOS
        // hides its platform binaries' markers, and on a host
        // without a user manager no tab scope holds them either.
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
        // Settle a stale "no manager" verdict BEFORE enumerating. The row's
        // recorded launch unit is the durable evidence that earns one
        // re-probe, and the sweep below would spend it anyway; spending it
        // after the globs had been skipped would kill only the names tmux
        // still listed, missing older generations and closed tabs, and then
        // publish "deleted" over them. A second negative stays final, so the
        // sweep's own re-probe costs nothing extra.
        if entry.scope.is_some() && !self.seams.scopes.available().await {
            self.seams.scopes.reprobe().await;
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

        // All refusal-prone, read-only preflights have passed: pane
        // ownership, terminal-tab rediscovery, and scope enumeration. Now
        // cancel in-flight uploads, still before the process sweep and every
        // destructive step below. A refused preflight therefore leaves the
        // transfer and its staged bytes available to the session, while a
        // successful Delete still stops new chunks before teardown can remove
        // the attachment directory. The lifecycle claim keeps new transfers
        // from staging between this cancellation and teardown (see
        // `stage_upload`).
        //
        // `abort_session_uploads` waits for the async tasks, not abandoned
        // blocking operations. A late publication can still race the
        // directory teardown below; cancellation itself is not rollback.
        abort_session_uploads(self, session_id, "the session was deleted").await;

        reap_process_tree(
            &self.seams.scopes,
            units,
            root_identity.into_iter().chain(other_roots),
            session_id,
            &SweepTarget::WholeSession,
            ScopeKillFailure::Refuse,
        )
        .await
        .map_err(TeardownError::Sweep)?;

        // Everything from here to the row's removal is fast (one tmux
        // round trip plus the session sink's bounded orderly shutdown, see
        // `DELETE_SINK_REAP_WAIT`, a few fail-closed removals, one sqlite
        // write) and
        // runs under `attachments`; the slow removal of the deleted
        // session's files waits until after the guard is released. The
        // guarded part mirrors the Attach
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
        // A live runtime-owned reaper still owns an output client, so the
        // session cannot be destroyed yet. A durable Failed entry differs:
        // its replacement barrier remains fail-closed, but no task is known
        // to be holding the client, so Delete may make the safe tmux progress
        // before returning its retained diagnostic.
        let output_reap_blocked = self.has_output_reap_for_session(session_id);
        if output_reap_blocked && forwarder_error.is_none() {
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
        let forwarder_error = forwarder_error;
        // Filled by the archive decisions below: notices for the reply, and
        // the checkout rows Delete gave up archiving, which its final
        // transaction releases instead of retiring.
        let mut notices: Vec<String> = Vec::new();
        let mut released: Vec<String> = Vec::new();
        let teardown: Result<crate::store::DeleteSettlement, String> = async {
            if output_reap_blocked {
                // Reaping is the only state that still proves a runtime task
                // owns an output client. Leave tmux and all durable state in
                // place until a later Delete observes a settled barrier.
                return Err(forwarder_error.expect("blocked output reap has a diagnostic"));
            }
            // A join failure has already become durable Failed evidence.
            // Killing tmux is safe progress for this failed attempt, but the
            // retained diagnostic prevents us from claiming deletion. Resolve
            // the target below, then return before artifacts or the row are
            // removed so a retry owns the remaining cleanup.
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
                    Ok(Some(tmux_name)) => match self
                        .tmux
                        .has_session_for_terminal_less_delete(&tmux_name)
                        .await
                    {
                        Ok(true) => Some(tmux_name),
                        Ok(false) => None,
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
                // Let the session sink's orderly shutdown finish while the
                // session still exists; see `DELETE_SINK_REAP_WAIT`. This
                // waits under the `attachments` lock, which every keystroke,
                // attach, detach and resize on this host goes through. That
                // is the brief, bounded local work SPEC.md ("Waiting between
                // operations on one host") lets terminals wait on: normally
                // one tmux round trip, the same kind of work Delete already
                // does under this lock, and the full limit only when tmux is
                // not answering, when terminal I/O is stalled anyway.
                let limit = self.timeouts.delete_sink_reap;
                if self.await_sink_reap(&tmux_name, limit).await == SinkReapWait::TimedOut {
                    warn!(
                        session = %session_id,
                        tmux = %tmux_name,
                        "the session's terminal-output client had not finished its orderly \
                         shutdown after {limit:?}; killing its tmux session anyway"
                    );
                }
                self.tmux
                    .kill_session(&tmux_name)
                    .await
                    .map_err(|e| format!("killing tmux session: {e:#}"))?;
            }
            if let Some(error) = forwarder_error {
                return Err(error);
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
                        // rename, no adoption. The user is told, not just
                        // the log (SPEC.md "Managed checkouts": Delete
                        // names the preserved path), and a path that could
                        // not be checked is reported as such rather than
                        // read as absent.
                        let path = PathBuf::from(&row.canonical_root).join(&row.original_basename);
                        let notice = match tokio::fs::symlink_metadata(&path).await {
                            Ok(_) => Some(format!(
                                "The managed checkout at {} was never fully set up, so Farhelm cannot \
                                 tell whether the folder there is its own; it was left untouched \
                                 for you to inspect.",
                                path.display()
                            )),
                            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                            Err(e) => Some(format!(
                                "The managed checkout at {} was never fully set up, and that path could \
                                 not be checked ({e}); if a folder is there, Farhelm left it \
                                 untouched for you to inspect.",
                                path.display()
                            )),
                        };
                        if let Some(notice) = notice {
                            warn!(
                                session = %session_id, path = %path.display(),
                                "delete retired an unresolved checkout plan; the directory at \
                                 the recorded path has no established ownership and is left untouched"
                            );
                            notices.push(notice);
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
                        // below governs. Reading the registry is a store
                        // failure and still fails the delete; everything
                        // the archive attempt itself can hit does not.
                        let registry = self.store.working_copy_rows().await.map_err(|e| {
                            format!("reading the registry for the reference check: {e:#}")
                        })?;
                        // Archiving never blocks Delete (SPEC.md "Managed checkouts", confirmed
                        // 2026-09-28). When the checkout cannot be archived safely, for any reason,
                        // the session is still deleted: the checkout is released from Farhelm's
                        // management, its folder stays where it is, and the reply carries a notice
                        // naming it. Before this, each of these failures kept the session, and
                        // several could never clear (a removed or remounted root, a filesystem
                        // without no-replace rename, a path too long to archive).
                        if let Err(skipped) = self
                            .archive_last_reference(&row, &registry, session_id)
                            .await
                        {
                            let path = row.canonical_path.clone().unwrap_or_else(|| {
                                PathBuf::from(&row.canonical_root)
                                    .join(&row.original_basename)
                                    .display()
                                    .to_string()
                            });
                            let ArchiveSkipped { reason, may_have_moved } = skipped;
                            // Name every place the folder can be. After a
                            // failed or unreconcilable move it may already be
                            // at its archive destination, and telling the user
                            // it stayed put would send them to an empty path.
                            let whereabouts = match may_have_moved {
                                None => "stays where it is".to_string(),
                                Some(Some(destination)) => {
                                    format!("may be there or at {destination}")
                                }
                                Some(None) => {
                                    "may be there or in its root's archive folder".to_string()
                                }
                            };
                            warn!(
                                session = %session_id, path = %path, reason = %reason,
                                whereabouts = %whereabouts,
                                "the deleted session's checkout could not be archived and is no \
                                 longer managed"
                            );
                            notices.push(format!(
                                "The managed checkout at {path} was not archived and {whereabouts}; \
                                 Farhelm no longer manages it. Reason: {reason}"
                            ));
                            released.push(row.id.clone());
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
                .delete_session_archiving_memberships(session_id, &released)
                .await
                .map_err(|e| format!("{e:#}"))
        }
        .await;

        let retired_checkouts = match teardown {
            Ok(crate::store::DeleteSettlement {
                retired,
                late_released,
            }) => {
                for late in late_released {
                    warn!(
                        session = %session_id, path = %late.path, reason = %late.reason,
                        "the deleted session's missing checkout could not be re-proved missing; \
                         it is no longer managed"
                    );
                    notices.push(format!(
                        "The managed checkout at {} was not archived and whatever is at that path now \
                         stays there; Farhelm no longer manages it. Reason: {}",
                        late.path, late.reason
                    ));
                }
                retired
            }
            Err(err_msg) => {
                // A returned failure is different from a process crash in
                // the quarantine window: the session row is still live, so
                // its attachments must be put back where readers can reach
                // them. The lifecycle claim and completed upload joins make
                // this rename safe. Keep the original fail-closed error;
                // restoration is best effort and a failure must be loud
                // without hiding the operation that refused the delete.
                if let Some(parked) = quarantined.take()
                    && let Err(restore_error) = crate::attachments::restore_quarantined(
                        &self.state_dir,
                        session_id,
                        &parked,
                    )
                    .await
                {
                    error!(
                        session = %session_id,
                        parked = %parked.display(),
                        error = %restore_error,
                        "failed to restore a retained session's quarantined attachments"
                    );
                }
                for (channel, notify) in &notify_detach {
                    notify_detached(
                        notify,
                        *channel,
                        format!("detached during a failed delete: {err_msg}"),
                        farhelm_proto::DetachCode::Other,
                    );
                }
                drop(attachments);
                return Err(TeardownError::FailClosed(err_msg));
            }
        };
        self.sessions.lock().await.remove(session_id);
        // The one removal point every delete reaches, on the task that owns
        // the delete; the handler's waiter can be abandoned before this.
        self.hint_sessions_changed();
        self.clear_failed_output_reaps_for_session(session_id);
        for (channel, notify) in &notify_detach {
            notify_detached(
                notify,
                *channel,
                "session deleted".to_string(),
                farhelm_proto::DetachCode::Other,
            );
        }
        drop(attachments);

        // Everything below removes on-disk state that now belongs to nothing,
        // and runs AFTER `attachments` is released. It is unbounded (a
        // session's uploaded files can be large and many), and that guard
        // serializes attach, input, resize and output flow control for every
        // session on the host, which must never wait on a Delete (SPEC.md
        // "Waiting between operations on one host"). Nothing here can race an
        // `Attach`: the row and the map entry are already gone.
        if let Some(gate) = self.seams.faults.deleted_session_cleanup_gate() {
            gate().await;
        }
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
        // Reports its hooks dropped and no pass applied yet go too, in the
        // same best-effort spirit: they describe a session that no longer
        // exists. A hook racing this delete can recreate the directory; the
        // next reconciliation pass removes it once it finds no session row.
        if let Some(reports) = crate::hook_report::session_dir(&self.state_dir, session_id)
            && let Err(e) = tokio::fs::remove_dir_all(&reports).await
            && e.kind() != std::io::ErrorKind::NotFound
        {
            warn!(
                session = %session_id, path = %reports.display(), error = %e,
                "could not remove a deleted session's waiting conversation reports"
            );
        }

        Ok((!notices.is_empty()).then(|| notices.join(" ")))
    }

    /// Archive the checkout whose last reference this Delete removes, or say
    /// why it could not be archived safely.
    ///
    /// `Err` carries what the user is told, not a failure of the Delete: the
    /// caller releases the checkout and completes the Delete with a notice
    /// (SPEC.md "Managed checkouts": archiving never blocks deleting its
    /// session). Every safety check that used to fail the Delete still
    /// decides whether the folder is MOVED: the move happens only when the
    /// root and the checkout still match what was recorded and no other
    /// registry row overlaps it; anything else leaves the folder alone.
    async fn archive_last_reference(
        &self,
        row: &crate::working_copies::WorkingCopyRow,
        registry: &[crate::working_copies::WorkingCopyRow],
        session_id: &str,
    ) -> Result<(), ArchiveSkipped> {
        // The corrupt-evidence check (Design E): overlapping managed paths
        // can only exist against the admission rule, and moving either
        // would act on inconsistent ownership evidence.
        let overlapping = registry
            .iter()
            .filter(|other| {
                other.id != row.id
                    && other.allocation_state != crate::working_copies::AllocationState::Retired
                    && other.canonical_path.as_deref().is_some_and(|other_path| {
                        crate::working_copies::path_overlaps(&row.canonical_path, other_path)
                    })
            })
            .count();
        if overlapping > 0 {
            let reason = "another active checkout record overlaps its path, so moving it would act \
                          on inconsistent ownership evidence";
            // The refusal proves no NEW move is safe, not that an earlier
            // interrupted one never happened, so a pending row still names
            // where it may have gone.
            if row.allocation_state == crate::working_copies::AllocationState::ArchivePending {
                return Err(ArchiveSkipped {
                    reason: reason.to_string(),
                    may_have_moved: Some(self.current_archive_destination(row).await),
                });
            }
            return Err(ArchiveSkipped::untouched(reason));
        }
        // A pending row first completes its crash recovery; a live row is
        // archived.
        if row.allocation_state == crate::working_copies::AllocationState::ArchivePending {
            let reconciled = self
                .store
                .reconcile_working_copy_archive(
                    &row.id,
                    self.seams.faults.archive_parent_sync().cloned(),
                )
                .await;
            let reconciled = match reconciled {
                Ok(outcome) => outcome,
                Err(e) if rename_refused(&e) => {
                    return Err(ArchiveSkipped::untouched(format!("{e:#}")));
                }
                Err(e) => {
                    return Err(ArchiveSkipped {
                        reason: format!("an earlier archive move could not be completed: {e:#}"),
                        may_have_moved: Some(self.current_archive_destination(row).await),
                    });
                }
            };
            match reconciled {
                crate::working_copies::ReconcileOutcome::Moved { destination } => {
                    warn!(session = %session_id, destination = %destination,
                        "the interrupted archive of a deleted session's checkout completed on retry");
                }
                crate::working_copies::ReconcileOutcome::MetadataComplete => {}
                crate::working_copies::ReconcileOutcome::SourceMissing => {
                    warn!(session = %session_id,
                        "a pending archive's source was already gone; deleting the record only");
                }
            }
            return Ok(());
        }
        // Missing-source cleanup needs the same root proof as a rename: a
        // replacement empty root can otherwise hide a still-owned checkout
        // elsewhere.
        crate::working_copies::verified_root(row).map_err(|e| {
            ArchiveSkipped::untouched(format!(
                "its checkout root no longer matches what was recorded: {e:#}"
            ))
        })?;
        match crate::working_copies::verify_identity(row) {
            Ok(crate::working_copies::IdentityStatus::Missing) => {
                // Missing source: a visible cleanup diagnostic, and metadata
                // deletion is permitted without moving anything.
                warn!(session = %session_id,
                    path = %row.canonical_path.as_deref().unwrap_or(""),
                    "the managed checkout's recorded directory is gone; deleting its ownership \
                     record without any move");
                Ok(())
            }
            Ok(crate::working_copies::IdentityStatus::Matches) => {
                match self
                    .store
                    .archive_move_working_copy(
                        &row.id,
                        self.seams.faults.archive_parent_sync().cloned(),
                    )
                    .await
                {
                    Ok(crate::working_copies::ArchiveOutcome::Archived { destination }) => {
                        warn!(session = %session_id, destination = %destination,
                            "the last reference to this checkout is being deleted; the directory \
                             moved to the archive");
                        Ok(())
                    }
                    Ok(crate::working_copies::ArchiveOutcome::SourceMissing) => {
                        warn!(session = %session_id,
                            "the checkout vanished between the identity check and the archive \
                             move; deleting only the record");
                        Ok(())
                    }
                    // A refused rename moved nothing and rolled its journal
                    // back, so the folder is where it was.
                    Err(e) if rename_refused(&e) => {
                        Err(ArchiveSkipped::untouched(format!("{e:#}")))
                    }
                    // Otherwise the move may have happened before a later
                    // durability step failed, so do not claim the folder
                    // stayed put.
                    Err(e) => Err(ArchiveSkipped {
                        reason: format!("the archive move failed: {e:#}"),
                        may_have_moved: Some(self.current_archive_destination(row).await),
                    }),
                }
            }
            Ok(crate::working_copies::IdentityStatus::DifferentObject) => {
                Err(ArchiveSkipped::untouched(
                    "the folder at that path is no longer the one this checkout recorded",
                ))
            }
            Ok(crate::working_copies::IdentityStatus::DeviceChangedUnconfirmed) => {
                Err(ArchiveSkipped::untouched(
                    "the folder's device number changed since it was recorded, as a reboot or \
                     remount can do on btrfs, NFS or overlayfs, and its filesystem records no \
                     creation time to confirm it is still the same folder",
                ))
            }
            Ok(status) => Err(ArchiveSkipped::untouched(format!(
                "its record has no captured identity ({status:?}), so Farhelm cannot tell the \
                 folder is its own"
            ))),
            Err(e) => Err(ArchiveSkipped::untouched(format!(
                "its identity could not be checked: {e:#}"
            ))),
        }
    }

    /// The archive destination currently journaled for `row`, as a full
    /// path, read fresh from the registry.
    ///
    /// Fresh because a move or a crash recovery journals its destination
    /// before renaming, and may re-journal a new name (a collision, or a
    /// foreign directory at the old name) before it fails, so the snapshot
    /// the caller holds can name a folder that is not ours. `None` when
    /// nothing is journaled or the read fails; the notice then says the
    /// destination is unknown rather than guessing.
    async fn current_archive_destination(
        &self,
        row: &crate::working_copies::WorkingCopyRow,
    ) -> Option<String> {
        let rows = self.store.working_copy_rows().await.ok()?;
        let name = rows
            .into_iter()
            .find(|current| current.id == row.id)?
            .archive_destination?;
        Some(journaled_destination_path(row, &name))
    }

    /// Close every terminal-output client this supervisor holds, each
    /// through its orderly no-output boundary, because the process is about
    /// to exit; give up after `budget`.
    ///
    /// This exists because a supervisor that simply exits closes all its
    /// tmux control clients at once, and tmux can abort its whole private
    /// server when a client that still has queued pane output sees EOF,
    /// ending every session on the host (BUGS.md: "Abrupt supervisor death
    /// can crash the private tmux server"). SIGKILL cannot be helped; a
    /// planned stop can, and the generated units' `KillMode=process` makes
    /// every planned stop and upgrade exactly that: a SIGTERM to the
    /// supervisor alone, while tmux and every session keep running. Remote
    /// uninstall is the exception: it ends the private tmux server on
    /// purpose, after systemd has forgotten that policy.
    ///
    /// First a stop boundary is established while holding `attachments`:
    /// from then on no new session sink is handed out and no attachment
    /// installs (see [`super::terminals::SinkRegistryState::stopping`]), so an attach racing
    /// the stop is refused rather than left streaming. Connection tasks keep
    /// running; they simply cannot create output clients any more. Then
    /// comes whole-session delete's orderly sequence with the session filter
    /// removed: every attachment is drained and its cleanup barrier
    /// published under that same lock hold, every forwarder is signalled
    /// before any is joined (so none can race the others), each forwarder
    /// crosses its own acknowledged no-output boundary, and each sink lease
    /// is dropped only after its forwarder finished, which hands the sink to
    /// its own orderly shutdown. Finally it waits until no sink is still
    /// owned and every runtime-owned reaper has settled. No detach notices
    /// are sent: the peers are about to lose the connection anyway.
    ///
    /// ONE budget covers all of it, including waiting for `attachments`
    /// itself, which an in-flight attach holds across tmux commands.
    /// Returns whether everything settled inside it. On expiry, one independent
    /// best-effort attempt switches the private server's control clients to
    /// no-output, with at most two additional seconds. It does not wait for
    /// their cleanup or turn an expired stop into a successful one. Errors from
    /// individual clients are recorded in the registries as usual.
    pub(crate) async fn shutdown_output_clients(&self, budget: std::time::Duration) -> bool {
        let settled = tokio::time::timeout(budget, async {
            let mut forwarders = tokio::task::JoinSet::new();
            {
                let mut attachments = self.attachments.lock().await;
                self.sinks.lock().expect("sink registry poisoned").stopping = true;
                let doomed: Vec<(AttachmentKey, ActiveAttach)> = attachments.drain().collect();
                for (key, old) in &doomed {
                    self.begin_forwarder_shutdown(key.clone(), old);
                }
                for (key, old) in doomed {
                    // `..` drops the input client (its control process goes
                    // with `kill_on_drop`; it carries no output) and the
                    // pause sender, exactly as whole-session delete does.
                    let ActiveAttach {
                        forwarder, sink, ..
                    } = old;
                    forwarders.spawn(async move {
                        let joined = forwarder.await;
                        drop(sink);
                        (key, joined)
                    });
                }
            }
            while let Some(joined) = forwarders.join_next().await {
                if let Ok((key, joined)) = joined {
                    let _ = self.record_forwarder_join(key, joined);
                }
            }
            self.wait_for_all_output_cleanup().await;
        })
        .await
        .is_ok();
        if !settled && let Err(error) = self.tmux.quiet_control_clients_before_exit().await {
            warn!(error = %error, "planned-stop output quiet-down failed; exiting anyway");
        }
        settled
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
    let state_path = crate::launch::preparation_state_path(state_dir, checkout_id);
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use tokio::sync::{mpsc, oneshot, watch};

    use super::super::core::tests::{StateDir, dummy_exe, entry_with, test_admission};
    use super::super::core::{CreateInputs, SupervisorSeams, SupervisorTimeouts};
    use super::super::terminals::{SessionSinkHandle, SessionSinkLease, SinkRegistryState};
    use super::*;
    use crate::store::{LastOutcome, StoredSession};
    use farhelm_proto::SessionLaunch;

    /// Seed a terminal-less, scoped session so teardown tests can isolate the
    /// cgroup verdict from tmux discovery and pane ownership.
    async fn scoped_session(
        scopes: crate::scope::ScopeManager,
        id: &str,
    ) -> (StateDir, Arc<Supervisor>, Arc<SessionEntry>) {
        scoped_session_with(
            SupervisorSeams {
                scopes: Arc::new(scopes),
                ..SupervisorSeams::default()
            },
            id,
        )
        .await
    }

    /// [`scoped_session`] with every seam chosen by the caller, for tests
    /// that also install a fault hook. `seams.scopes` must be set: the
    /// seeded entry names a launch scope.
    async fn scoped_session_with(
        seams: SupervisorSeams,
        id: &str,
    ) -> (StateDir, Arc<Supervisor>, Arc<SessionEntry>) {
        scoped_session_with_timeouts(seams, SupervisorTimeouts::default(), id).await
    }

    /// [`scoped_session_with`] with the supervisor's timeouts chosen too, for
    /// a test that has to reach a budget's expiry.
    async fn scoped_session_with_timeouts(
        seams: SupervisorSeams,
        timeouts: SupervisorTimeouts,
        id: &str,
    ) -> (StateDir, Arc<Supervisor>, Arc<SessionEntry>) {
        let state = StateDir::new();
        let unit = crate::scope::unit_name(id, 0).expect("a UUID id must name a scope unit");
        let sup = Supervisor::new_with_seams(state.path(), dummy_exe(), timeouts, seams)
            .await
            .expect("supervisor");
        sup.store
            .insert_session(
                StoredSession {
                    conversation_source: None,
                    capture_ownership_version: 0,
                    omp_reporter_asset: None,
                    omp_launch_program: None,
                    launch_hooked: false,
                    id: id.to_string(),
                    parent: None,
                    title: id.to_string(),
                    created_at: 1_700_000_000,
                    last_activity_at: 1_700_000_000,
                    last_work_started_at: 0,
                    creation_seq: 0,
                    cwd: "/tmp".to_string(),
                    launch: farhelm_proto::SessionLaunch::Legacy {
                        invocation: "agent".to_string(),
                        agent_kind: farhelm_proto::AgentKind::Generic,
                        resume_template: None,
                    },
                    tmux_name: format!("fh-{id}"),
                    pane: String::new(),
                    outcome: LastOutcome::Running,
                    canonical_cwd: None,
                    captured_conversation: None,
                    generation: 0,
                    launch_scoped: true,
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

    /// Delete removes the deleted session's files (uploaded attachments,
    /// preparation state, hook trace, reports its hooks dropped) only after
    /// releasing the supervisor-wide `attachments` guard.
    ///
    /// Why it matters: that guard serializes attach, input, resize and output
    /// flow control for every session on the host, and the file removal is
    /// unbounded, so doing it under the guard froze every other session's
    /// terminal for as long as a large deletion took (SPEC.md "Waiting between
    /// operations on one host"). Specified: when Delete reaches its file
    /// cleanup the guard is already free, and the Delete then completes.
    #[farhelm_testtrace::test]
    async fn delete_releases_the_attachments_guard_before_removing_files() {
        let id = uuid::Uuid::new_v4().to_string();
        let (reached_tx, reached_rx) = oneshot::channel::<()>();
        let (release_tx, release_rx) = oneshot::channel::<()>();
        let reached = Arc::new(std::sync::Mutex::new(Some(reached_tx)));
        let release = Arc::new(tokio::sync::Mutex::new(Some(release_rx)));
        let gate: super::super::core::DeletedSessionCleanupGate = Arc::new(move || {
            let reached = Arc::clone(&reached);
            let release = Arc::clone(&release);
            Box::pin(async move {
                if let Some(tx) = reached.lock().expect("gate mutex").take() {
                    let _ = tx.send(());
                }
                if let Some(rx) = release.lock().await.take() {
                    let _ = rx.await;
                }
            })
        });
        let mut seams = SupervisorSeams {
            scopes: Arc::new(working_scopes()),
            ..SupervisorSeams::default()
        };
        seams.faults.deleted_session_cleanup_gate = Some(gate);
        let (state, sup, entry) = scoped_session_with(seams, &id).await;
        // Files the cleanup must remove, so the test can tell cleanup that
        // ran before the guard's release from cleanup that runs after it.
        let upload = crate::attachments::session_dir(state.path(), &id).join("upload.bin");
        std::fs::create_dir_all(upload.parent().expect("upload has a parent"))
            .expect("create the session's attachment directory");
        std::fs::write(&upload, b"uploaded").expect("seed an uploaded file");
        let hook_log = crate::service::core::hook_log_path(state.path(), &id);
        std::fs::create_dir_all(hook_log.parent().expect("hook log has a parent"))
            .expect("create the hook-log directory");
        std::fs::write(&hook_log, b"hook").expect("seed a hook trace");
        let reports =
            crate::hook_report::session_dir(state.path(), &id).expect("a UUID names a dir");
        std::fs::create_dir_all(&reports).expect("create the drop directory");
        std::fs::write(reports.join("latest.json"), b"{}").expect("seed a waiting report");
        let quarantine = crate::attachments::attachments_root(state.path()).join(".quarantine");
        let quarantined_uploads = || -> Vec<std::path::PathBuf> {
            std::fs::read_dir(&quarantine)
                .map(|entries| {
                    entries
                        .filter_map(Result::ok)
                        .filter(|entry| entry.file_name().to_string_lossy().starts_with(&id))
                        .map(|entry| entry.path().join("upload.bin"))
                        .filter(|path| path.exists())
                        .collect()
                })
                .unwrap_or_default()
        };

        let delete = tokio::spawn({
            let sup = Arc::clone(&sup);
            let id = id.clone();
            async move {
                let admission = test_admission(&sup).await;
                sup.teardown_session(&entry, &id, admission).await
            }
        });
        tokio::time::timeout(std::time::Duration::from_secs(30), reached_rx)
            .await
            .expect("delete must reach its post-commit file cleanup")
            .expect("the cleanup gate reports arrival");
        assert!(
            sup.attachments.try_lock().is_ok(),
            "delete must not hold the supervisor-wide attachments guard while removing files"
        );
        assert!(
            sup.store.session(&id).await.expect("read row").is_none(),
            "test premise: the row is already gone when file cleanup starts"
        );
        assert_eq!(
            quarantined_uploads().len(),
            1,
            "the uploaded file is still waiting in quarantine when the guard is already free"
        );
        assert!(
            hook_log.exists(),
            "the hook trace is still present when the guard is already free"
        );

        release_tx.send(()).expect("release the cleanup gate");
        assert!(
            delete.await.expect("delete task").is_ok(),
            "the delete completes once its file cleanup runs"
        );
        assert!(
            quarantined_uploads().is_empty(),
            "the cleanup removes the quarantined upload"
        );
        assert!(!hook_log.exists(), "the cleanup removes the hook trace");
        assert!(!reports.exists(), "the cleanup removes the waiting reports");
    }

    /// Stop fails when the session's recorded scope cannot even be checked
    /// because the systemd user manager is not usable now.
    ///
    /// Why it matters: the launch ran in a scope that may still hold a
    /// daemon the process sweep cannot see; a manager that stopped answering
    /// (both the startup probe and the one re-probe failed) is no evidence
    /// the scope is gone, so reporting a clean stop would hide survivors
    /// (SPEC.md "Lifecycle operations"). Specified: with both probes negative
    /// and a recorded scope, `stop_live_agent` returns a sweep failure.
    #[farhelm_testtrace::test]
    async fn stop_refuses_when_a_recorded_scope_cannot_be_checked() {
        let id = uuid::Uuid::new_v4().to_string();
        let (_state, sup, entry) = scoped_session(
            crate::scope::ScopeManager::fake_reprobing(false, false, Arc::new(|_| {})),
            &id,
        )
        .await;

        let result = super::super::sweep::stop_live_agent(&sup, &id, &entry, None).await;

        assert!(
            matches!(result, Err(super::super::sweep::StopFailure::Sweep(_))),
            "an uncheckable recorded scope must fail the stop"
        );
    }

    /// Stop reports a failure when it cannot confirm that the agent's cgroup
    /// scope is gone.
    ///
    /// Why it matters: a still-loaded scope may hold a daemon the process sweep
    /// cannot see, so reporting a clean stop would hide survivors; SPEC.md's
    /// Lifecycle operations (confirmed 2026-09-28) makes such an operation fail
    /// visibly, as Delete already did. Specified: with a scope manager whose
    /// kills never confirm, `stop_live_agent` returns a sweep failure.
    #[farhelm_testtrace::test]
    async fn stop_refuses_when_the_scope_cannot_be_confirmed() {
        let id = uuid::Uuid::new_v4().to_string();
        let (_state, sup, entry) = scoped_session(
            crate::scope::ScopeManager::fake_failing_kills(Arc::new(|_| {})),
            &id,
        )
        .await;

        let result = super::super::sweep::stop_live_agent(&sup, &id, &entry, None).await;

        assert!(
            matches!(result, Err(super::super::sweep::StopFailure::Sweep(_))),
            "an unconfirmed scope must fail the stop"
        );
    }

    /// A session with a real tmux session and one attachment whose sink is a
    /// test-controlled fake, for the Delete-versus-sink-shutdown tests.
    ///
    /// The fake sink's task waits for the shutdown request and reports it on
    /// `shutdown_seen`, which is the moment Delete has released the last sink
    /// reference and the reaper is running. With `finish_on_release` it then
    /// waits for `release`, records whether the tmux session still existed,
    /// and ends; without it, it never ends, like a client tmux never answers.
    struct SinkDeleteFixture {
        _state: StateDir,
        sup: Arc<Supervisor>,
        entry: Arc<SessionEntry>,
        id: String,
        tmux_name: String,
        shutdown_seen: oneshot::Receiver<()>,
        release: oneshot::Sender<()>,
        saw_session: oneshot::Receiver<anyhow::Result<bool>>,
    }

    async fn sink_delete_fixture(
        timeouts: SupervisorTimeouts,
        finish_on_release: bool,
    ) -> SinkDeleteFixture {
        let id = uuid::Uuid::new_v4().to_string();
        let (state, sup, entry) = scoped_session_with_timeouts(
            SupervisorSeams {
                scopes: Arc::new(working_scopes()),
                ..SupervisorSeams::default()
            },
            timeouts,
            &id,
        )
        .await;
        let tmux_name = format!("fh-{id}");
        let pane = sup
            .tmux
            .create_session(
                &tmux_name,
                "/",
                80,
                24,
                &[],
                &["sleep".to_string(), "60".to_string()],
            )
            .await
            .expect("fixture premise: tmux session must be created before Delete");
        let input = sup
            .tmux
            .open_input_client(&tmux_name, &pane)
            .await
            .expect("fixture premise: input client must attach");

        let (shutdown, shutdown_rx) = oneshot::channel::<()>();
        let (shutdown_seen_tx, shutdown_seen) = oneshot::channel::<()>();
        let (release, released) = oneshot::channel::<()>();
        let (saw_session_tx, saw_session) = oneshot::channel::<anyhow::Result<bool>>();
        let (sink_state, _sink_state_rx) = watch::channel(None);
        let sink_task = tokio::spawn({
            let sup = Arc::clone(&sup);
            let tmux_name = tmux_name.clone();
            async move {
                let _ = shutdown_rx.await;
                let _ = shutdown_seen_tx.send(());
                if !finish_on_release {
                    std::future::pending::<()>().await;
                }
                let _ = released.await;
                let _ = saw_session_tx.send(sup.tmux.has_session(&tmux_name).await);
                Ok::<_, anyhow::Error>(())
            }
        });
        let handle = Arc::new(SessionSinkHandle {
            tmux_name: tmux_name.clone(),
            task: Some(sink_task),
            shutdown: Some(shutdown),
            state: sink_state,
        });
        sup.sinks.lock().expect("sink registry").insert(
            tmux_name.clone(),
            super::super::terminals::SinkRegistryEntry::Live(Arc::downgrade(&handle)),
        );
        let sink = SessionSinkLease::new(handle, Arc::clone(&sup.sinks));
        let (notify, _notify_rx) = mpsc::channel(2);
        let (forwarder_shutdown, _shutdown_observer) = watch::channel(false);
        let (forwarder_cleanup, cleanup_rx) = watch::channel(None);
        let (pause, _pause_rx) = watch::channel(None);
        sup.attachments.lock().await.insert(
            crate::service::terminals::AttachmentKey::new(
                &id,
                crate::service::terminals::TerminalId::Agent,
            ),
            ActiveAttach {
                channel: 7,
                lease: "test".to_string(),
                notify,
                forwarder: tokio::spawn(async {}),
                forwarder_shutdown,
                forwarder_cleanup: cleanup_rx,
                input,
                pause,
                pane_death: super::super::terminals::PaneDeath::new(&tmux_name, &pane).0,
                sink,
            },
        );
        drop(forwarder_cleanup);
        SinkDeleteFixture {
            _state: state,
            sup,
            entry,
            id,
            tmux_name,
            shutdown_seen,
            release,
            saw_session,
        }
    }

    /// Delete lets the session sink's orderly shutdown finish before it kills
    /// the session's tmux session, and leaves no sink behind.
    ///
    /// Why it matters: the shutdown turns the sink's output off through a
    /// control-mode exchange with tmux, which only works while the session
    /// exists. Killed first, tmux answered "can't find client" while the
    /// client stayed alive behind its unread output, and the reaper retried
    /// forever: a leaked tmux client and a tmux command every five seconds
    /// until the supervisor restarted. Spec: while the reaper is still
    /// running, Delete has not killed the session; once it finishes, Delete
    /// kills it and succeeds, and the session's sink entry is gone. The test
    /// holds the reaper itself, so the order is observed deterministically
    /// rather than by reproducing the original race.
    #[farhelm_testtrace::test]
    async fn delete_lets_the_sink_shut_down_before_killing_tmux() {
        let fixture = sink_delete_fixture(SupervisorTimeouts::default(), true).await;
        let SinkDeleteFixture {
            _state,
            sup,
            entry,
            id,
            tmux_name,
            shutdown_seen,
            release,
            saw_session,
        } = fixture;
        let delete = tokio::spawn({
            let sup = Arc::clone(&sup);
            async move {
                let admission = test_admission(&sup).await;
                sup.teardown_session(&entry, &id, admission).await
            }
        });
        tokio::time::timeout(std::time::Duration::from_secs(10), shutdown_seen)
            .await
            .expect("Delete must release the session sink")
            .expect("the sink reports its shutdown request");
        assert!(
            matches!(
                sup.sinks.lock().expect("sink registry").get(&tmux_name),
                Some(super::super::terminals::SinkRegistryEntry::Reaping(_))
            ),
            "premise: the released sink is being reaped"
        );
        // sleep-ok: observation window in which a Delete that did not wait for the sink would already have killed tmux
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        assert!(
            !delete.is_finished(),
            "Delete must wait for the sink's shutdown"
        );

        release.send(()).expect("release the sink's shutdown");
        assert!(
            saw_session
                .await
                .expect("the sink reports")
                .expect("probe the tmux session from the sink"),
            "the sink finished while its tmux session still existed"
        );
        assert!(
            delete.await.expect("delete task").is_ok(),
            "Delete succeeds once the sink is gone"
        );
        assert!(
            sup.sinks
                .lock()
                .expect("sink registry")
                .get(&tmux_name)
                .is_none(),
            "no sink entry is left behind"
        );
        assert!(
            !sup.tmux
                .has_session_for_terminal_less_delete(&tmux_name)
                .await
                .expect("tmux liveness after Delete"),
            "Delete kills the tmux session"
        );
    }

    /// A sink whose orderly shutdown never finishes delays Delete only by
    /// the bounded wait, after which Delete kills the session and succeeds.
    ///
    /// Why it matters: the wait exists for a shutdown that normally takes
    /// one exchange with tmux, but it runs under the lock every terminal on
    /// the host shares, so a client tmux never answers must not turn into a
    /// Delete that fails or hangs (P3 of the plan that added the wait: kill
    /// as before and log). Spec: with the budget expired, the session is
    /// killed, the row removed and Delete reports success, while the sink's
    /// entry is still `Reaping`, which the fallback deliberately leaves to
    /// its reaper.
    #[farhelm_testtrace::test]
    async fn delete_kills_tmux_when_the_sink_shutdown_outlasts_its_wait() {
        let fixture = sink_delete_fixture(
            SupervisorTimeouts {
                delete_sink_reap: std::time::Duration::from_millis(100),
                ..SupervisorTimeouts::default()
            },
            false,
        )
        .await;
        let admission = test_admission(&fixture.sup).await;
        let deleted = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            fixture
                .sup
                .teardown_session(&fixture.entry, &fixture.id, admission),
        )
        .await
        .expect("Delete must not wait past its budget");
        assert!(deleted.is_ok(), "Delete succeeds after the bounded wait");
        assert!(
            !fixture
                .sup
                .tmux
                .has_session_for_terminal_less_delete(&fixture.tmux_name)
                .await
                .expect("tmux liveness after Delete"),
            "Delete kills the tmux session after the wait"
        );
        assert!(
            fixture
                .sup
                .store
                .session(&fixture.id)
                .await
                .expect("read the row")
                .is_none(),
            "Delete removes the session's row"
        );
        assert!(
            matches!(
                fixture
                    .sup
                    .sinks
                    .lock()
                    .expect("sink registry")
                    .get(&fixture.tmux_name),
                Some(super::super::terminals::SinkRegistryEntry::Reaping(_))
            ),
            "the unfinished shutdown stays with its reaper"
        );
        drop(fixture.release);
    }

    /// A failed forwarder join must still kill the session's tmux server, but
    /// the first Delete remains visibly incomplete so a retry can finish the
    /// durable row cleanup. The fixture uses a real control client and tmux
    /// session, then cancels only the forwarder task to reproduce its
    /// JoinError boundary without changing the test process environment.
    #[farhelm_testtrace::test]
    async fn failed_forwarder_join_kills_tmux_then_retry_deletes_the_row() {
        let id = uuid::Uuid::new_v4().to_string();
        let (_state, sup, entry) = scoped_session(working_scopes(), &id).await;
        let tmux_name = format!("fh-{id}");
        let pane = sup
            .tmux
            .create_session(
                &tmux_name,
                "/",
                80,
                24,
                &[],
                &["sleep".to_string(), "60".to_string()],
            )
            .await
            .expect("fixture premise: tmux session must be created before Delete");
        assert!(
            sup.tmux
                .has_session(&tmux_name)
                .await
                .expect("fixture premise: probe tmux session")
        );
        let input = sup
            .tmux
            .open_input_client(&tmux_name, &pane)
            .await
            .expect("fixture premise: input client must attach");
        let (_shutdown, _shutdown_rx) = oneshot::channel();
        let (sink_state, _sink_state_rx) = watch::channel(None);
        let sink_task = tokio::spawn(async { Ok::<_, anyhow::Error>(()) });
        let sink = SessionSinkLease::new(
            Arc::new(SessionSinkHandle {
                tmux_name: tmux_name.clone(),
                task: Some(sink_task),
                shutdown: Some(_shutdown),
                state: sink_state,
            }),
            Arc::new(std::sync::Mutex::new(SinkRegistryState::default())),
        );
        let (notify, _notify_rx) = mpsc::channel(2);
        let (forwarder_shutdown, _shutdown_observer) = watch::channel(false);
        let (forwarder_cleanup, cleanup_rx) = watch::channel(None);
        let (pause, _pause_rx) = watch::channel(None);
        let forwarder = tokio::spawn(async {
            std::future::pending::<()>().await;
        });
        // Inject the JoinError boundary without a panic; the teardown wrapper
        // observes this cancellation as a failed forwarder join.
        forwarder.abort();
        sup.attachments.lock().await.insert(
            crate::service::terminals::AttachmentKey::new(
                &id,
                crate::service::terminals::TerminalId::Agent,
            ),
            ActiveAttach {
                channel: 7,
                lease: "test".to_string(),
                notify,
                forwarder,
                forwarder_shutdown,
                forwarder_cleanup: cleanup_rx,
                input,
                pause,
                pane_death: super::super::terminals::PaneDeath::new(&tmux_name, &pane).0,
                sink,
            },
        );
        drop(forwarder_cleanup);

        let first = sup
            .teardown_session(&entry, &id, test_admission(&sup).await)
            .await;
        let first_failed = matches!(
            first,
            Err(TeardownError::FailClosed(message)) if message.contains("forwarder")
        );
        assert!(
            first_failed,
            "first Delete must surface the injected forwarder failure"
        );
        assert!(
            !sup.tmux
                .has_session_for_terminal_less_delete(&tmux_name)
                .await
                .expect("tmux liveness after partial Delete"),
            "first Delete must kill tmux even though it retains the row"
        );
        assert!(
            sup.store
                .session(&id)
                .await
                .expect("read retained row")
                .is_some(),
            "first Delete must retain the row for retry"
        );

        let entry = sup
            .sessions
            .lock()
            .await
            .get(&id)
            .cloned()
            .expect("retained row reloads for retry");
        let retry = sup
            .teardown_session(&entry, &id, test_admission(&sup).await)
            .await;
        assert!(
            retry.is_ok(),
            "retry must complete after tmux teardown progress"
        );
        assert!(
            sup.store
                .session(&id)
                .await
                .expect("read deleted row")
                .is_none(),
            "successful retry must remove the durable row"
        );
        assert!(
            !sup.output_reaps
                .lock()
                .expect("output-reap registry")
                .keys()
                .any(|key| key.session == id),
            "successful Delete must prune failed output-reap evidence"
        );
    }

    /// A row-removal refusal occurs after attachment quarantine, so the
    /// retained session must regain its published files before the supervisor
    /// can report the failure. Reopening the supervisor exercises startup
    /// reconciliation too: known-session attachments must survive that sweep,
    /// while a later successful retry still removes them with the row.
    #[farhelm_testtrace::test]
    async fn failed_row_delete_restores_attachments_before_startup_reconciliation() {
        let id = uuid::Uuid::new_v4().to_string();
        let (state, sup, entry) = scoped_session(working_scopes(), &id).await;
        crate::attachments::ensure_session_dirs(state.path(), &id)
            .await
            .expect("attachment fixture");
        let attachment = crate::attachments::session_dir(state.path(), &id).join("kept.txt");
        tokio::fs::write(&attachment, b"retained bytes")
            .await
            .expect("attachment fixture bytes");
        {
            let conn = rusqlite::Connection::open(state.path().join("supervisor.db"))
                .expect("open raw store connection");
            conn.execute_batch(
                "CREATE TRIGGER refuse_delete_after_quarantine BEFORE DELETE ON sessions \
                 BEGIN SELECT RAISE(ABORT, 'refused after quarantine'); END;",
            )
            .expect("plant the post-quarantine refusal");
        }

        let first = sup
            .teardown_session(&entry, &id, test_admission(&sup).await)
            .await;
        assert!(
            matches!(first, Err(TeardownError::FailClosed(message)) if message.contains("refused after quarantine")),
            "the deliberate row refusal must remain visible"
        );
        assert!(
            sup.store
                .session(&id)
                .await
                .expect("read retained row")
                .is_some(),
            "a failed delete must retain the session row"
        );
        assert_eq!(
            tokio::fs::read(&attachment)
                .await
                .expect("restored attachment"),
            b"retained bytes",
            "a failed delete must restore the quarantined attachment"
        );

        drop(sup);
        {
            let conn = rusqlite::Connection::open(state.path().join("supervisor.db"))
                .expect("reopen raw store connection");
            conn.execute_batch("DROP TRIGGER refuse_delete_after_quarantine")
                .expect("remove the injected refusal before retry");
        }
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
        .expect("reopen retained session");
        assert_eq!(
            tokio::fs::read(&attachment)
                .await
                .expect("startup must preserve retained attachment"),
            b"retained bytes",
            "startup reconciliation must not remove a known session's files"
        );
        let entry = sup
            .sessions
            .lock()
            .await
            .get(&id)
            .cloned()
            .expect("retained row reloads for retry");
        assert!(
            sup.teardown_session(&entry, &id, test_admission(&sup).await)
                .await
                .is_ok(),
            "successful retry"
        );
        assert!(
            sup.store
                .session(&id)
                .await
                .expect("read deleted row")
                .is_none(),
            "successful retry must remove the retained row"
        );
        assert!(
            !crate::attachments::session_dir(state.path(), &id).exists(),
            "successful retry must remove the attachment directory"
        );
    }

    /// Delete re-probes a stale "no manager" verdict before enumerating the
    /// session's scopes, so the units only the manager can list are killed.
    ///
    /// Why it matters: an older generation's scope or a closed tab's scope
    /// is found only by asking the manager, and it may hold a daemon that
    /// left the process tree and scrubbed its environment. Enumerating under
    /// the stale verdict skipped that listing; the sweep's later re-probe
    /// then killed only the recorded unit and Delete removed the row, leaving
    /// the daemon running with nothing left to find it. Spec: the recorded
    /// launch unit earns one re-probe before enumeration, and a successful
    /// one puts the manager-listed units in the kill set.
    #[farhelm_testtrace::test]
    async fn delete_reprobes_a_stale_negative_verdict_before_listing_scopes() {
        let id = uuid::Uuid::new_v4().to_string();
        let older_generation =
            crate::scope::unit_name(&id, 7).expect("a UUID id must name a scope unit");
        let observed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = {
            let observed = Arc::clone(&observed);
            Arc::new(move |op: &crate::scope::ScopeOp| {
                observed.lock().unwrap().push(op.clone());
            }) as crate::scope::ScopeOpSink
        };
        let (_state, sup, entry) = scoped_session(
            crate::scope::ScopeManager::fake_reprobing_with_matching_units_vanishing(
                false,
                true,
                vec![older_generation.clone()],
                3,
                sink,
            ),
            &id,
        )
        .await;
        assert!(
            !sup.seams.scopes.available().await,
            "fixture premise: the cached verdict is a stale negative"
        );

        assert!(
            sup.teardown_session(&entry, &id, test_admission(&sup).await)
                .await
                .is_ok(),
            "a re-probed manager that confirms its units gone allows Delete"
        );

        let observed = observed.lock().expect("op sink mutex poisoned");
        // The fake lists units whatever its verdict, so the kill alone would
        // also pass with the re-probe left to the sweep. The ordering is the
        // distinguishing observable: the permitted re-probe (the second
        // probe) must come before the first listing.
        let reprobe_at = observed
            .iter()
            .enumerate()
            .filter(|(_, op)| matches!(op, crate::scope::ScopeOp::Probe))
            .nth(1)
            .map(|(at, _)| at)
            .expect("the recorded launch unit must earn one re-probe");
        let first_list_at = observed
            .iter()
            .position(|op| matches!(op, crate::scope::ScopeOp::List(_)))
            .expect("Delete must ask the manager for the session's units");
        assert!(
            reprobe_at < first_list_at,
            "the stale verdict must be re-probed before listing: {observed:?}"
        );
        assert!(
            observed.iter().any(|op| matches!(
                op,
                crate::scope::ScopeOp::Kill { unit, .. } if *unit == older_generation
            )),
            "the manager-listed older generation must be killed: {observed:?}"
        );
    }

    /// Spec: a delete hints connected helms from the teardown itself, with no
    /// request handler involved.
    ///
    /// Why: the delete's reply waiter is connection-owned and can be aborted
    /// (a connection's shutdown timeout does exactly that) while the
    /// supervisor-owned teardown still finishes. A hint sent by the waiter
    /// would be lost with it, leaving the removal to the helm's backstop poll.
    #[farhelm_testtrace::test]
    async fn a_teardown_hints_without_its_request_waiter() {
        let id = uuid::Uuid::new_v4().to_string();
        let (_state, sup, entry) = scoped_session(working_scopes(), &id).await;
        let mut hints = crate::service::hints::test_support::HintProbe::attach(&sup).await;
        assert!(
            sup.teardown_session(&entry, &id, test_admission(&sup).await)
                .await
                .is_ok(),
            "fixture premise: the teardown succeeds"
        );
        hints.expect_hint("the removed session").await;
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
                    capture_ownership_version: 0,
                    omp_reporter_asset: None,
                    omp_launch_program: None,
                    launch_hooked: false,
                    id: id.clone(),
                    parent: None,
                    title: "previous scope".to_string(),
                    created_at: 1_700_000_000,
                    last_activity_at: 1_700_000_000,
                    last_work_started_at: 0,
                    creation_seq: 0,
                    cwd: "/tmp".to_string(),
                    launch: farhelm_proto::SessionLaunch::Legacy {
                        invocation: "agent".to_string(),
                        agent_kind: farhelm_proto::AgentKind::Generic,
                        resume_template: None,
                    },
                    tmux_name: format!("fh-{id}"),
                    pane: String::new(),
                    outcome: LastOutcome::Running,
                    canonical_cwd: None,
                    captured_conversation: None,
                    generation: 4,
                    launch_scoped: false,
                },
                None,
            )
            .await
            .expect("seed the session being deleted");
        let mut entry = entry_with(None, LastOutcome::Running);
        entry.info.id = id.clone();

        let Ok(_) = sup
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
                    capture_ownership_version: 0,
                    omp_reporter_asset: None,
                    omp_launch_program: None,
                    launch_hooked: false,
                    id: id.to_string(),
                    parent: None,
                    title: id.to_string(),
                    created_at: 1_700_000_000,
                    last_activity_at: 1_700_000_000,
                    last_work_started_at: 0,
                    creation_seq: 0,
                    cwd: cwd.to_string(),
                    launch: farhelm_proto::SessionLaunch::Legacy {
                        invocation: "agent".to_string(),
                        agent_kind: farhelm_proto::AgentKind::Generic,
                        resume_template: None,
                    },
                    tmux_name: format!("fh-{id}"),
                    pane: String::new(),
                    outcome: LastOutcome::Running,
                    canonical_cwd: Some(cwd.to_string()),
                    captured_conversation: None,
                    generation: 0,
                    launch_scoped: true,
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

    /// A checkout the teardown saw as missing, whose absence no longer holds
    /// when Delete commits, is released with a notice instead of rolling the
    /// Delete back.
    ///
    /// Why it matters: the final transaction re-proves a missing source before dropping its record,
    /// and the filesystem can change in between (the folder reappears, the root is unmounted).
    /// Failing there would make an archive outcome block Delete after all, which SPEC.md "Managed
    /// checkouts" rules out. Specified: settling the last reference to an allocated checkout whose
    /// folder is present (the state a reappeared folder leaves) deletes the session and the record,
    /// leaves the folder untouched, and reports the path as released late.
    #[farhelm_testtrace::test]
    async fn a_missing_checkout_that_reappears_before_commit_is_released_not_fatal() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let root = state.path().join("workroot");
        std::fs::create_dir_all(&root).expect("the checkout root");
        let checkout_id;
        {
            let conn = sup.store.conn.lock();
            let planned = crate::working_copies::record_planned(
                &conn,
                &crate::working_copies::PlannedWorkingCopy {
                    id: uuid::Uuid::new_v4().to_string(),
                    canonical_root: root.to_string_lossy().into_owned(),
                    repo_owner: "octo".to_string(),
                    repo_name: "back".to_string(),
                    original_basename: "back".to_string(),
                    origin_session_id: "s-back".to_string(),
                    root_identity: None,
                    preparation_snapshot: None,
                },
            )
            .expect("record the checkout plan");
            crate::working_copies::allocate(&conn, &planned.id, None)
                .expect("allocate the checkout");
            checkout_id = planned.id;
        }
        let path = root.join("back");
        std::fs::write(path.join("file.txt"), "contents").expect("seed the folder");
        seeded_session(&sup, "s-back", &path.to_string_lossy()).await;

        let settlement = sup
            .store
            .delete_session_archiving_memberships("s-back", &[])
            .await
            .expect("the present folder is released, not a failed Delete");
        assert_eq!(settlement.late_released.len(), 1);
        assert_eq!(
            settlement.late_released[0].path,
            path.to_string_lossy(),
            "the late release names the folder's recorded path"
        );
        assert!(sup.store.session("s-back").await.unwrap().is_none());
        assert!(
            !sup.store
                .working_copy_rows()
                .await
                .unwrap()
                .iter()
                .any(|row| row.id == checkout_id),
            "the checkout is released from Farhelm's management"
        );
        assert_eq!(
            std::fs::read_to_string(path.join("file.txt")).unwrap(),
            "contents",
            "the folder is untouched"
        );
    }

    /// E1 (Design E): the last-reference delete archives the checkout —
    /// exact contents appear ONCE under farhelm-archived-working-copies,
    /// the source path is gone, the registry row retires — while a
    /// multi-member delete moves NOTHING, and an unmanaged directory is
    /// never touched.
    ///
    /// Preparation state and its lock survive owner deletion with a borrower
    /// and a failed last Delete; successful final retirement removes both.
    /// This keeps cleanup tied to durable checkout lifetime.
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
            let conn = sup.store.conn.lock();
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
            let conn = sup.store.conn.lock();
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
            // A filesystem that immediately gives the stranger a different
            // inode never needed fillers and has no filler directory to remove.
            if made_fillers != 0 {
                std::fs::remove_dir_all(
                    accepted
                        .canonical_path
                        .parent()
                        .unwrap()
                        .join("stranger-fx"),
                )
                .expect("drop the identity shifters");
            }
            assert!(
                differs,
                "the fixture could not mint a stranger with a different identity"
            );
        }
        let entry = seeded_session(&sup, "s-swap", &root.join("swap").to_string_lossy()).await;
        let result = sup
            .teardown_session(&entry, "s-swap", test_admission(&sup).await)
            .await;
        // Archiving never blocks Delete (SPEC.md "Managed checkouts"):
        // the session goes, the checkout is released, and the reply names
        // the folder left in place. What must never happen is the move.
        let Ok(Some(notice)) = result else {
            panic!("a foreign object at the recorded source still deletes, with a notice");
        };
        assert!(
            notice.contains(&root.join("swap").display().to_string()),
            "the notice names the folder left in place: {notice}"
        );
        assert!(
            sup.store.session("s-swap").await.expect("read").is_none(),
            "the session is deleted"
        );
        assert!(
            !sup.store
                .working_copy_rows()
                .await
                .expect("read the registry")
                .into_iter()
                .any(|row| row.id == checkout_id),
            "the checkout is released from Farhelm's management"
        );
        assert!(
            root.join("swap").exists(),
            "the stranger at the recorded path is untouched"
        );
    }

    /// Planned rows deliberately lack an accepted path. Explicit Delete must
    /// still name the candidate it leaves untouched, without adopting its inode
    /// or treating that diagnostic path as authority to archive unknown content.
    ///
    /// The name goes into the Delete's own notice as well as the log: SPEC.md
    /// "Managed checkouts" makes the user, who deleted the session from
    /// the UI and never sees the supervisor log, the one who must learn a
    /// folder was left behind.
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
            let conn = sup.store.conn.lock();
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
        let Ok(Some(notice)) = sup
            .teardown_session(&entry, "planned-origin", test_admission(&sup).await)
            .await
        else {
            panic!("explicit Delete retires only the unresolved metadata, with a notice");
        };
        assert!(
            notice.contains(unknown.to_str().unwrap()) && notice.contains("left untouched"),
            "the Delete's own result names the preserved path: {notice}"
        );
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
                    let conn = sup.store.conn.lock();
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
                let conn = sup.store.conn.lock();
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

    /// A moved or replaced checkout root never makes Delete touch the
    /// checkout, and never blocks the Delete either.
    ///
    /// Why it matters: a removed, recreated or symlink-swapped root used to
    /// make every Delete (and restart) of the session fail forever. SPEC.md
    /// "Managed checkouts" makes archiving never block Delete; the
    /// safety that remains is that the checkout, wherever it now lives, is
    /// neither moved nor treated as gone. Specified: the delete succeeds with
    /// a notice, the checkout's record is released, and the real checkout
    /// under the moved root is untouched.
    #[farhelm_testtrace::test]
    async fn deleting_with_a_replaced_root_releases_the_checkout_untouched() {
        use std::os::unix::fs::MetadataExt;
        for symlink_root in [false, true] {
            let state = StateDir::new();
            let root = state.path().join("workroot");
            let parked = state.path().join("parked");
            let foreign = state.path().join("foreign");
            std::fs::create_dir(&root).unwrap();
            let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
                .await
                .unwrap();
            let checkout_id = uuid::Uuid::new_v4().to_string();
            let accepted = {
                let conn = sup.store.conn.lock();
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
            let entry = sup
                .sessions
                .lock()
                .await
                .get("missing-origin")
                .cloned()
                .unwrap();
            // Archiving never blocks Delete (SPEC.md "Managed checkouts"): a replaced root
            // completes the delete with a notice and releases the checkout. What must never happen
            // is treating the checkout as gone and touching it, wherever it now lives.
            let Ok(Some(notice)) = sup
                .teardown_session(&entry, "missing-origin", test_admission(&sup).await)
                .await
            else {
                panic!("a replaced root still deletes the session, with a notice");
            };
            assert!(
                notice.contains("bar"),
                "the notice names the checkout left behind: {notice}"
            );
            assert!(sup.store.session("missing-origin").await.unwrap().is_none());
            assert!(
                !sup.store
                    .working_copy_rows()
                    .await
                    .unwrap()
                    .iter()
                    .any(|row| row.id == checkout_id),
                "the checkout is released from Farhelm's management"
            );
            let metadata = std::fs::metadata(parked.join("bar")).unwrap();
            assert_eq!(
                (metadata.dev(), metadata.ino()),
                accepted.identity,
                "the real checkout under the moved root is untouched"
            );
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
            let _ = (&preparation, &preparation_lock);
        }
    }

    /// An interrupted archive refused for overlapping registry records still
    /// completes the Delete with a notice naming its journaled destination.
    ///
    /// Why it matters: the overlap makes any further move unsafe, but the
    /// interrupted rename may already have happened, and Delete forgets the
    /// record right after. Saying the checkout stayed at its source would
    /// hide where it went. Specified: with a pending archive whose rename
    /// completed and another active record overlapping its path, Delete
    /// succeeds, both folders are untouched, the notice names the journaled
    /// destination and does not claim the checkout stayed put, and the
    /// checkout is released.
    #[farhelm_testtrace::test]
    async fn an_overlap_refusal_of_a_pending_archive_names_its_destination() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let root = state.path().join("workroot");
        std::fs::create_dir(&root).unwrap();
        let archive_root = root.join(crate::working_copies::ARCHIVE_DIR_NAME);
        std::fs::create_dir(&archive_root).unwrap();
        let source = root.join("checkout");
        let destination = archive_root.join("checkout-journaled");
        let checkout_id = uuid::Uuid::new_v4().to_string();
        {
            let conn = sup.store.conn.lock();
            for (id, basename, origin) in [
                (checkout_id.as_str(), "checkout", "overlapped"),
                ("overlapping", "other", "other-origin"),
            ] {
                crate::working_copies::record_planned(
                    &conn,
                    &crate::working_copies::PlannedWorkingCopy {
                        id: id.to_string(),
                        canonical_root: root.to_str().unwrap().into(),
                        repo_owner: "acme".into(),
                        repo_name: basename.into(),
                        original_basename: basename.into(),
                        origin_session_id: origin.into(),
                        root_identity: None,
                        preparation_snapshot: None,
                    },
                )
                .unwrap();
                crate::working_copies::allocate(&conn, id, None).unwrap();
            }
            // Corrupt evidence the admission rule never produces: another
            // active record inside this checkout's path.
            conn.execute(
                "UPDATE working_copies SET canonical_path = ?2 WHERE id = ?1",
                rusqlite::params![
                    "overlapping",
                    source.join("inner").to_string_lossy().into_owned()
                ],
            )
            .unwrap();
            // An interrupted archive whose rename went through.
            conn.execute(
                "UPDATE working_copies SET allocation_state = ?2, archive_destination = ?3 \
                 WHERE id = ?1",
                rusqlite::params![
                    checkout_id,
                    crate::working_copies::AllocationState::ArchivePending.as_str(),
                    "checkout-journaled",
                ],
            )
            .unwrap();
        }
        std::fs::write(source.join("owned"), b"uncommitted content").unwrap();
        std::fs::rename(&source, &destination).unwrap();
        let entry = seeded_session(&sup, "overlapped", source.to_str().unwrap()).await;

        let Ok(Some(notice)) = sup
            .teardown_session(&entry, "overlapped", test_admission(&sup).await)
            .await
        else {
            panic!("an overlap refusal still deletes the session, with a notice");
        };
        assert!(
            notice.contains(&destination.display().to_string())
                && !notice.contains("stays where it is"),
            "the notice names where the checkout may be: {notice}"
        );
        assert_eq!(
            std::fs::read(destination.join("owned")).unwrap(),
            b"uncommitted content"
        );
        assert!(
            root.join("other").is_dir(),
            "the other checkout is untouched"
        );
        assert!(sup.store.session("overlapped").await.unwrap().is_none());
        assert!(
            !sup.store
                .working_copy_rows()
                .await
                .unwrap()
                .iter()
                .any(|row| row.id == checkout_id),
            "the checkout is released from Farhelm's management"
        );
    }

    /// When the filesystem refuses the archive rename outright, Delete
    /// completes with a notice that the checkout stayed where it was.
    ///
    /// Why it matters: a refusal (an unsupported no-overwrite rename, a
    /// permission problem) moves nothing, so a notice saying the folder "may
    /// be" in the archive would send the user looking for a folder that was
    /// never moved. Specified, with a read-only archive folder producing a
    /// real `EACCES`: Delete succeeds, the notice says the checkout stays
    /// where it is and names its path, the folder and its contents are
    /// untouched, the archive is empty, and the checkout is released.
    #[farhelm_testtrace::test]
    async fn a_refused_archive_rename_leaves_the_checkout_with_a_notice() {
        use std::os::unix::fs::PermissionsExt;
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let root = state.path().join("workroot");
        std::fs::create_dir(&root).unwrap();
        let archive_root = root.join(crate::working_copies::ARCHIVE_DIR_NAME);
        std::fs::create_dir(&archive_root).unwrap();
        let checkout_id = uuid::Uuid::new_v4().to_string();
        {
            let conn = sup.store.conn.lock();
            crate::working_copies::record_planned(
                &conn,
                &crate::working_copies::PlannedWorkingCopy {
                    id: checkout_id.clone(),
                    canonical_root: root.to_str().unwrap().into(),
                    repo_owner: "acme".into(),
                    repo_name: "checkout".into(),
                    original_basename: "checkout".into(),
                    origin_session_id: "refused".into(),
                    root_identity: None,
                    preparation_snapshot: None,
                },
            )
            .unwrap();
            crate::working_copies::allocate(&conn, &checkout_id, None).unwrap();
        }
        let source = root.join("checkout");
        std::fs::write(source.join("owned"), b"uncommitted content").unwrap();
        let entry = seeded_session(&sup, "refused", source.to_str().unwrap()).await;
        std::fs::set_permissions(&archive_root, std::fs::Permissions::from_mode(0o555)).unwrap();
        // A privileged test process ignores directory permissions, and then
        // there is no refusal to observe.
        if std::fs::create_dir(archive_root.join("probe")).is_ok() {
            std::fs::set_permissions(&archive_root, std::fs::Permissions::from_mode(0o755))
                .unwrap();
            eprintln!("SKIPPED: directory permissions are not enforced for this process");
            return;
        }

        let result = sup
            .teardown_session(&entry, "refused", test_admission(&sup).await)
            .await;
        std::fs::set_permissions(&archive_root, std::fs::Permissions::from_mode(0o755)).unwrap();
        let Ok(Some(notice)) = result else {
            panic!("a refused archive rename still deletes the session, with a notice");
        };
        assert!(
            notice.contains("stays where it is") && notice.contains(&source.display().to_string()),
            "the notice says the folder did not move and names it: {notice}"
        );
        assert_eq!(
            std::fs::read(source.join("owned")).unwrap(),
            b"uncommitted content"
        );
        assert_eq!(std::fs::read_dir(&archive_root).unwrap().count(), 0);
        assert!(sup.store.session("refused").await.unwrap().is_none());
        assert!(
            !sup.store
                .working_copy_rows()
                .await
                .unwrap()
                .iter()
                .any(|row| row.id == checkout_id),
            "the checkout is released from Farhelm's management"
        );
    }

    /// A crash recovery that moves the checkout under a fresh name and then
    /// fails its durability barrier completes the Delete with a notice naming
    /// the name the folder actually reached.
    ///
    /// Why it matters: recovery re-journals a new destination when a foreign
    /// directory holds the old one, so the destination recorded before the
    /// recovery names a folder that is not ours. A notice built from that
    /// stale record would send the user to a stranger's folder and omit where
    /// the checkout went, just before Delete forgets the record. Specified:
    /// with a pending archive whose journaled name is occupied by a foreign
    /// directory and a failing parent barrier, Delete succeeds, its notice
    /// names the checkout's new location, the foreign directory is untouched,
    /// and the checkout is released.
    #[farhelm_testtrace::test]
    async fn recovery_barrier_failure_names_the_rejournaled_destination() {
        let state = StateDir::new();
        let root = state.path().join("workroot");
        std::fs::create_dir(&root).unwrap();
        let source = root.join("checkout");
        let archive_root = root.join(crate::working_copies::ARCHIVE_DIR_NAME);
        std::fs::create_dir(&archive_root).unwrap();
        let sync: crate::working_copies::ArchiveParentSync = Arc::new(|_, _| {
            Err(std::io::Error::other(
                "injected archive parent sync failure",
            ))
        });
        let sup = Supervisor::new_with_seams(
            state.path(),
            dummy_exe(),
            SupervisorTimeouts::default(),
            SupervisorSeams {
                faults: crate::service::FaultHooks {
                    archive_parent_sync: Some(sync),
                    ..crate::service::FaultHooks::default()
                },
                ..SupervisorSeams::default()
            },
        )
        .await
        .unwrap();
        let checkout_id = uuid::Uuid::new_v4().to_string();
        {
            let conn = sup.store.conn.lock();
            crate::working_copies::record_planned(
                &conn,
                &crate::working_copies::PlannedWorkingCopy {
                    id: checkout_id.clone(),
                    canonical_root: root.to_str().unwrap().into(),
                    repo_owner: "acme".into(),
                    repo_name: "checkout".into(),
                    original_basename: "checkout".into(),
                    origin_session_id: "recovering".into(),
                    root_identity: None,
                    preparation_snapshot: None,
                },
            )
            .unwrap();
            crate::working_copies::allocate(&conn, &checkout_id, None).unwrap();
            // The state an interrupted archive leaves: journaled, not moved.
            conn.execute(
                "UPDATE working_copies SET allocation_state = ?2, archive_destination = ?3 \
                 WHERE id = ?1",
                rusqlite::params![
                    checkout_id,
                    crate::working_copies::AllocationState::ArchivePending.as_str(),
                    "checkout-old",
                ],
            )
            .unwrap();
        }
        let foreign = archive_root.join("checkout-old");
        std::fs::create_dir(&foreign).unwrap();
        std::fs::write(foreign.join("theirs"), b"not ours").unwrap();
        let entry = seeded_session(&sup, "recovering", source.to_str().unwrap()).await;

        let Ok(Some(notice)) = sup
            .teardown_session(&entry, "recovering", test_admission(&sup).await)
            .await
        else {
            panic!("a failed recovery barrier still deletes the session, with a notice");
        };
        let moved: Vec<_> = std::fs::read_dir(&archive_root)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path != &foreign)
            .collect();
        assert_eq!(
            moved.len(),
            1,
            "recovery moved the checkout under a new name"
        );
        assert!(
            notice.contains(&moved[0].display().to_string()),
            "the notice names where the checkout went: {notice}"
        );
        assert!(
            !notice.contains(&foreign.display().to_string()),
            "the notice must not send the user to the foreign folder: {notice}"
        );
        assert_eq!(std::fs::read(foreign.join("theirs")).unwrap(), b"not ours");
        assert!(sup.store.session("recovering").await.unwrap().is_none());
        assert!(
            !sup.store
                .working_copy_rows()
                .await
                .unwrap()
                .iter()
                .any(|row| row.id == checkout_id),
            "the checkout is released from Farhelm's management"
        );
    }

    /// A failed durability barrier after the archive rename completes the
    /// Delete with a notice that does not claim the folder stayed put.
    ///
    /// Why it matters: this failure used to keep the session and its journal
    /// for a retry that could repeat the failure forever; SPEC.md "Fresh
    /// GitHub checkouts" makes archiving never block Delete. Because the
    /// rename already happened, the notice must name the archive destination the folder may be at,
    /// not claim it stayed put. Specified, for either failing parent barrier: the
    /// delete succeeds with such a notice, the checkout is released, and the
    /// moved checkout is intact exactly once in the archive. This drives the
    /// actual fsync seam, not a failure next to it.
    #[farhelm_testtrace::test]
    async fn archive_parent_sync_failure_completes_delete_with_a_notice() {
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
                    faults: crate::service::FaultHooks {
                        archive_parent_sync: Some(sync.clone()),
                        ..crate::service::FaultHooks::default()
                    },
                    ..SupervisorSeams::default()
                },
            )
            .await
            .unwrap();
            let checkout_id = uuid::Uuid::new_v4().to_string();
            let owned_identity = {
                let conn = sup.store.conn.lock();
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
            let result = sup
                .teardown_session(&entry, "sync-origin", test_admission(&sup).await)
                .await;
            // Archiving never blocks Delete (SPEC.md "Managed checkouts"). The rename has already
            // happened when the durability barrier fails, so the notice must not claim the folder
            // stayed put.
            let Ok(Some(notice)) = result else {
                panic!("a failed archive barrier still deletes the session, with a notice");
            };
            assert!(
                notice.contains("injected archive parent sync failure")
                    && !notice.contains("stays where it is"),
                "{notice}"
            );
            let expected_calls = if fail_archive_parent {
                vec![archive_root.clone()]
            } else {
                vec![archive_root.clone(), root.clone()]
            };
            assert_eq!(*calls.lock().unwrap(), expected_calls);
            assert!(sup.store.session("sync-origin").await.unwrap().is_none());
            assert!(
                !sup.store
                    .working_copy_rows()
                    .await
                    .unwrap()
                    .iter()
                    .any(|row| row.id == checkout_id),
                "the checkout is released from Farhelm's management"
            );
            let moved: Vec<_> = std::fs::read_dir(&archive_root)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .collect();
            assert_eq!(moved.len(), 1, "the checkout was moved exactly once");
            assert!(
                notice.contains(&moved[0].display().to_string()),
                "the notice names the archive destination the folder reached: {notice}"
            );
            let metadata = std::fs::metadata(&moved[0]).unwrap();
            assert_eq!((metadata.dev(), metadata.ino()), owned_identity);
            assert_eq!(
                std::fs::read(moved[0].join("owned")).unwrap(),
                b"uncommitted content"
            );
            assert!(!source.exists(), "the failure came after the real rename");
            let _ = (&failing, &mut entry, &mut sup);
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
            let conn = sup.store.conn.lock();
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
            let conn = sup.store.conn.lock();
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
                faults: crate::service::FaultHooks {
                    create_directory_waiting: Some(Arc::new(move || signal.notify_one())),
                    ..crate::service::FaultHooks::default()
                },
                ..SupervisorSeams::default()
            },
        )
        .await
        .expect("supervisor");
        let root = state.path().join("workroot");
        std::fs::create_dir_all(&root).expect("the checkout root");
        let working_copy_id;
        {
            let conn = sup.store.conn.lock();
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
            launch: SessionLaunch::plain_command("agent"),
            title: None,
            cols: 80,
            rows: 24,
        };
        if create_first {
            let guards = sup.admit_create(None).await.unwrap();
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

    // -----------------------------------------------------------------
    // Orderly stop: `shutdown_output_clients`
    // -----------------------------------------------------------------

    /// A registered, live session-sink lease backed by a fake task that ends
    /// as soon as it is asked to, standing in for a real sink client so the
    /// stop boundary can be exercised without tmux.
    fn fake_live_sink(sup: &Supervisor, tmux_name: &str) -> SessionSinkLease {
        let (shutdown, shutdown_rx) = oneshot::channel::<()>();
        let (state, _state_rx) = watch::channel(None);
        let task = tokio::spawn(async move {
            let _ = shutdown_rx.await;
            Ok::<_, anyhow::Error>(())
        });
        let handle = Arc::new(SessionSinkHandle {
            tmux_name: tmux_name.to_string(),
            task: Some(task),
            shutdown: Some(shutdown),
            state,
        });
        sup.sinks.lock().expect("sink registry").insert(
            tmux_name.to_string(),
            super::super::terminals::SinkRegistryEntry::Live(Arc::downgrade(&handle)),
        );
        SessionSinkLease::new(handle, Arc::clone(&sup.sinks))
    }

    /// Spec: a stop does not report completion while any session sink is
    /// still owned, refuses new sinks from the moment it starts, and
    /// completes once the last owner lets go.
    ///
    /// Why: the stop exists so that no output-bearing client is closed
    /// abruptly at exit. An attach that got its sink just before the stop
    /// holds a live lease outside the attachment map; reporting completion
    /// then would let the process exit with that sink still streaming. And
    /// a sink handed out after the stop began would be a new client nothing
    /// is going to close in order.
    #[farhelm_testtrace::test]
    async fn a_stop_waits_for_live_sinks_and_refuses_new_ones() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let lease = fake_live_sink(&sup, "fh-held");

        assert!(
            !sup.shutdown_output_clients(std::time::Duration::from_millis(200))
                .await,
            "a stop must not complete while a sink lease is still held"
        );
        assert!(sup.is_stopping());
        let refused = sup
            .ensure_session_sink("fh-new")
            .await
            .err()
            .expect("no new sink may be handed out once stopping");
        assert!(
            format!("{refused:#}").contains(super::super::terminals::SUPERVISOR_STOPPING),
            "unexpected refusal: {refused:#}"
        );

        drop(lease);
        assert!(
            sup.shutdown_output_clients(std::time::Duration::from_secs(5))
                .await,
            "the stop must complete once the last lease is released"
        );
    }

    /// Spec: when the last owner of a live sink lets go while a stop is
    /// waiting, the stop keeps waiting until that sink's reaper finishes;
    /// it does not return in the gap between "no live sink" and "reaper
    /// done".
    ///
    /// Why: the lease destructor turns `Live` into `Reaping` and only then
    /// schedules the orderly shutdown. A stop that saw the sink as neither
    /// would exit before that shutdown disabled the client's output. The
    /// reaper here is held until the test releases it, so a premature
    /// return is visible deterministically.
    #[farhelm_testtrace::test]
    async fn a_stop_waits_for_the_reaper_a_released_sink_leaves_behind() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let (release, released) = oneshot::channel::<()>();
        let (shutdown, _shutdown_rx) = oneshot::channel::<()>();
        let (sink_state, _sink_state_rx) = watch::channel(None);
        let handle = Arc::new(SessionSinkHandle {
            tmux_name: "fh-held-reaper".to_string(),
            task: Some(tokio::spawn(async move {
                let _ = released.await;
                Ok::<_, anyhow::Error>(())
            })),
            shutdown: Some(shutdown),
            state: sink_state,
        });
        sup.sinks.lock().expect("sink registry").insert(
            "fh-held-reaper".to_string(),
            super::super::terminals::SinkRegistryEntry::Live(Arc::downgrade(&handle)),
        );
        let lease = SessionSinkLease::new(handle, Arc::clone(&sup.sinks));

        let stop = tokio::spawn({
            let sup = Arc::clone(&sup);
            async move {
                sup.shutdown_output_clients(std::time::Duration::from_secs(10))
                    .await
            }
        });
        drop(lease);
        // sleep-ok: observation window in which a stop that lost track of the reaper would already have returned
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        assert!(
            !stop.is_finished(),
            "the stop must still be waiting for the released sink's reaper"
        );
        release.send(()).expect("release the reaper");
        assert!(
            stop.await.expect("stop task"),
            "the stop must complete once the reaper finishes"
        );
    }

    /// Spec: the stop budget includes waiting for the `attachments` lock.
    ///
    /// Why: an in-flight attach holds that lock across tmux commands that
    /// have no timeout of their own, so a budget that started only after
    /// acquiring it would not bound the stop at all; a wedged tmux would
    /// hang a desktop quit or outlast systemd's stop timeout.
    #[farhelm_testtrace::test]
    async fn the_stop_budget_covers_the_attachments_lock() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let held = sup.attachments.lock().await;
        assert!(
            !sup.shutdown_output_clients(std::time::Duration::from_millis(100))
                .await,
            "a stop must give up while the attachments lock is held past its budget"
        );
        drop(held);
    }
}
