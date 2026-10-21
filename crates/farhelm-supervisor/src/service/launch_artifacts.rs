//! Generation-scoped launch specs, failure sentinels and wrapper waypoints.
//!
//! A launch spec is written before an agent execs; a sentinel exists only
//! when the exec FAILED — the shim records the errno before exiting
//! (PLAN_M3.md item 10). The empty waypoint proves the login shell reached
//! Farhelm's wrapper step. All three live under the private state directory.
//! `crate::launch` owns their on-disk shape, the shim-side write, and the
//! raw sentinel read through `LaunchArtifacts`; what lives here is their
//! service side — the async read wrapper, outcome classification, and the
//! removal and sweep helpers. A never-started error may also retain a bounded terminal excerpt,
//! using the launch's process-local fence and tmux's atomic capture guard.
//! This never runs on the list path. See `service::sweep` for the process-tree
//! side of a launch's failure classification.

use super::core::{SessionEntry, Supervisor};
use crate::launch::LaunchArtifacts;
use crate::store::{LastOutcome, SessionStore};
use anyhow::Context;
use std::path::Path;
use std::sync::atomic::Ordering;
use tracing::{debug, warn};

/// Append terminal evidence only after the caller has proved never-started.
///
/// The fence belongs to this entry's launch, not its reusable pane. Reload and
/// ambiguous starts deliberately have none. tmux revalidates ownership,
/// generation and deadness in the same synchronous command group as capture;
/// an obsolete observer can therefore never quote a later launch's output.
///
/// `capture_available` belongs to one observer pass. Any failed capture spends
/// the rest of its budget, including a timeout, while ordinary classification
/// keeps progressing. A missing fence or refused guard costs no capture error.
/// Stop uses a separate budget and calls this only for a pane already dead.
pub(crate) async fn append_launch_excerpt(
    sup: &super::core::Supervisor,
    entry: &super::core::SessionEntry,
    mut detail: String,
    capture_available: &mut bool,
) -> String {
    if !*capture_available {
        return detail;
    }
    let fence = entry
        .run
        .launch_excerpt
        .lock()
        .expect("launch-excerpt mutex poisoned")
        .clone();
    let (Some(fence), Some(terminal)) = (fence, entry.terminal.as_ref()) else {
        return detail;
    };
    match sup
        .tmux
        .capture_launch_excerpt(&terminal.tmux_name, &terminal.pane, &fence)
        .await
    {
        Ok(Some(excerpt)) if !excerpt.is_empty() => {
            detail.push_str("; last terminal output: ");
            detail.push_str(&excerpt);
        }
        Ok(_) => {}
        Err(error) => {
            *capture_available = false;
            // The diagnostic is optional. In particular, tmux failure must
            // never discard the launch evidence already proved on disk.
            debug!(session = %entry.info.id, error = %format!("{error:#}"),
                "could not capture never-started terminal output; omitting excerpts for the rest of this pass");
        }
    }
    detail
}

/// The explanation for a launch that never reached the exec shim at all,
/// or `None` when this is not that shape (PLAN_M3.md item 10).
///
/// The login shell and, on scoped launches, systemd wrapper run before the
/// shim can write its sentinel. A dead pane with an unconsumed spec proves
/// neither reached the agent. The waypoint distinguishes those steps for
/// new scoped commands; old scoped specs retain the two-suspect wording
/// because absence of a marker they never wrote proves nothing.
///
/// The recognizable shape has three parts, and every one is load-bearing:
///
/// 1. **A DEAD pane, not an absent one.** `remain-on-exit` keeps a pane
///    whose command exited, so a failed wrapper leaves the pane there and
///    dead. A pane that is GONE means the tmux window (or the whole server)
///    was destroyed — which also strands an unconsumed spec, from a launch
///    that was merely interrupted rather than failed. Conflating the two
///    reported perfectly ordinary sessions as `error`; the distinction is
///    what this classifier actually rests on. Callers pass `pane_dead`.
/// 2. **No sentinel.** The shim's own report outranks every inference,
///    including this one, so callers ask only after reading no sentinel.
/// 3. **An unconsumed spec.** The shim unlinks its spec as soon as it has
///    read it, so a spec still present under a dead pane means the shim
///    never ran at all.
///
/// An unscoped command goes directly from the shell to the shim, so its
/// unconsumed spec already identifies the shell, including on older builds.
/// Callers keep annotated Stop outcomes and unattributed panes outside this
/// inference. Exit codes explain the failed step; they never establish it.
///
/// A stat failure is inconclusive, not a proven absence. Callers may preserve
/// their last-resort exit inference, but must not settle future reads from it:
/// an unreadable state directory cannot prove that the shim consumed its spec.
pub(crate) async fn wrapper_failure_detail(
    state_dir: &Path,
    id: &str,
    generation: i64,
    scoped: bool,
    pane_dead: bool,
    exit_code: Option<i32>,
) -> anyhow::Result<Option<String>> {
    if !pane_dead {
        return Ok(None);
    }
    let artifacts = LaunchArtifacts::for_launch(state_dir, id, generation);
    let bytes = match tokio::fs::read(artifacts.spec()).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    // Decode only the compatibility bit, rather than materializing the
    // credential-bearing launch just to classify which step failed.
    #[derive(serde::Deserialize)]
    struct Evidence {
        #[serde(default)]
        scope_waypoint_expected: Option<bool>,
    }
    let waypoint_expected = if scoped {
        serde_json::from_slice::<Evidence>(&bytes)
            .context("reading launch waypoint expectation")?
            .scope_waypoint_expected
    } else {
        Some(false)
    };
    if scoped && waypoint_expected.is_none() {
        return Ok(Some(
            "the agent was never started: the launch never reached farhelm's exec shim, so \
             something before it — the transient cgroup scope wrapper, or the login shell \
             itself — exited first"
                .to_string(),
        ));
    }
    let wrapper_reached =
        waypoint_expected == Some(true) && tokio::fs::try_exists(artifacts.waypoint()).await?;
    let code = exit_code.map_or_else(String::new, |code| format!(" (code {code})"));
    Ok(Some(if wrapper_reached {
        format!(
            "the agent was never started: the systemd scope wrapper exited{code} before \
             starting the agent; the host's systemd user manager may have refused the scope"
        )
    } else {
        format!(
            "the agent was never started: your login shell exited{code} before it ran \
             Farhelm's launch command; a startup file such as .bashrc or .zshrc usually \
             exits or replaces the shell"
        )
    }))
}

/// Read launch evidence off the async worker for observation and reload.
///
/// A missing file is an ordinary negative read. Even a usually cheap filesystem
/// call can block on its underlying storage, so spawn_blocking keeps it from
/// delaying terminal forwarding on the same worker. Replies never call this;
/// the timer logs read failures and retries while lists remain available.
pub(crate) async fn read_launch_sentinel(
    state_dir: &Path,
    id: &str,
    generation: i64,
) -> anyhow::Result<Option<String>> {
    let state_dir = state_dir.to_path_buf();
    let id = id.to_string();
    tokio::task::spawn_blocking(move || {
        crate::launch::read_launch_sentinel(&state_dir, &id, generation)
    })
    .await
    .context("launch sentinel read task panicked")?
}

/// Whether a launch sentinel discovered NOW could still change `outcome` —
/// the read-time mirror of `Transition::apply`'s own `SentinelError` rule
/// (`store.rs`), kept as one function so the two places that decide
/// whether reading the file is even worth attempting (`core`'s
/// `reload_sessions` and the periodic observer) can never drift from what
/// the store would actually do with the reading once it is offered.
///
/// `false` only for an already-`Error` row (idempotent — nothing to gain)
/// and for a GENUINELY annotated `Exited` (a real stop: retained
/// knowledge, not an inference a sentinel could outrank). `true` for
/// everything else, INCLUDING `Interrupted` and an unannotated `Exited` —
/// PLAN_M3.md item 3 requires a late-discovered sentinel to still
/// supersede both, because neither is anything more than an inference
/// from an ordinary dead-or-vanished pane, exactly the evidence class a
/// sentinel is defined to beat. Periodic observation also checks its
/// generation-local negative-read latch: after owned pane death or a boot
/// interruption, successful negative reads cannot acquire new shim evidence.
pub(crate) fn sentinel_could_still_apply(outcome: &LastOutcome) -> bool {
    !matches!(
        outcome,
        LastOutcome::Error { .. }
            | LastOutcome::Exited {
                annotation: Some(_),
                ..
            }
    )
}

/// Remove a durably classified launch's artifacts after preserving create
/// replay evidence. Callers must own durable-write authority.
///
/// The sentinel and credential-bearing spec can survive a crash after Error
/// commits. For generation zero, the sentinel may also be the last proof
/// that the original create reached a terminal: Error alone proves nothing,
/// because preterminal refusals use that outcome too. Settle matching Pending
/// creates before unlinking an actual sentinel; retain the artifacts if its read
/// or settlement fails, or its durable generation no longer matches.
///
/// Startup and periodic observers share this gate, including already-Error
/// paths. Returns true only after evidence preservation and all removals
/// succeeded (missing files count as removed). Observers then stop retrying
/// for this generation; failures remain retryable. A later generation cannot
/// settle the original create from its own sentinel.
pub(crate) async fn cleanup_launch_artifacts(
    state_dir: &Path,
    store: &SessionStore,
    id: &str,
    generation: i64,
) -> bool {
    // Only the first launch can settle its original create. A real sentinel
    // may be its last acceptance evidence after tmux and its scope vanish;
    // Error alone cannot replace that proof because preterminal refusals use
    // the same outcome. Keep the artifacts if preservation cannot commit.
    if generation == 0 {
        let preserve = async {
            if read_launch_sentinel(state_dir, id, generation)
                .await?
                .is_some()
            {
                store.settle_sentinel_create_before_cleanup(id).await
            } else {
                Ok(true)
            }
        }
        .await;
        match preserve {
            Ok(true) => {}
            Ok(false) => {
                debug!(session = %id, generation,
                    "deferring launch cleanup: the sentinel's error generation is not durable");
                return false;
            }
            Err(error) => {
                warn!(session = %id, generation, error = %format!("{error:#}"),
                    "retaining launch artifacts until accepted-create evidence is durable");
                return false;
            }
        }
    }
    LaunchArtifacts::for_launch(state_dir, id, generation)
        .remove_settled_best_effort()
        .await
}

/// Retire a durable Error's artifacts only after preservation and removal succeed.
/// Callers establish the Error outcome before requesting cleanup; failed reads,
/// settlements and unlinks leave the generation-local latch clear for retry.
/// Rename shares this latch, while relaunch replaces it.
///
/// Tick and wake share sampling admission, but Stop holds its lifecycle claim
/// independently. Concurrent callers may both attempt cleanup: this is a
/// success cache, not an in-flight claim or an exactly-once guarantee.
pub(super) async fn cleanup_error_once(sup: &Supervisor, entry: &SessionEntry) {
    if sup.may_record()
        && !entry.run.launch_cleanup_done.load(Ordering::Relaxed)
        && cleanup_launch_artifacts(&sup.state_dir, &sup.store, &entry.info.id, entry.generation)
            .await
    {
        entry.run.launch_cleanup_done.store(true, Ordering::Relaxed);
    }
}

/// Clear the launch artifacts sitting at ONE launch's generation-named
/// paths, FAIL-CLOSED, before something writes its own there.
///
/// Reached from exactly one caller now that per-launch paths are
/// generation-scoped: a create RETAKING an interrupted attempt's
/// identities (PLAN_M3.md item 6), which reuses the reservation's session
/// id and therefore its generation-0 paths. A sentinel that survived into
/// that relaunch would be read as evidence about a launch that has not
/// happened yet, painting a perfectly good agent as `error`; a stale spec
/// is a credential-bearing file with no owner. So a cleanup that cannot be
/// CONFIRMED aborts the relaunch rather than proceeding: destroying
/// evidence is bad, but launching on top of evidence this process could
/// not remove is worse.
///
/// A RESTART needs none of this — its new generation names files nothing
/// has ever written (`launch::spec_path_for_launch`), which is the whole
/// reason those paths carry the generation.
///
/// The sentinel goes first: it is the one whose survival changes a
/// classification, so if only one removal gets to run, that is
/// the one worth having run.
pub(crate) async fn clear_launch_artifacts_fail_closed(
    state_dir: &Path,
    id: &str,
    generation: i64,
) -> Result<(), String> {
    LaunchArtifacts::for_launch(state_dir, id, generation)
        .clear_for_retake_fail_closed()
        .await
}

impl LaunchArtifacts {
    /// Retain every file-removal failure before a refused create settles its key.
    /// The caller has confirmed tmux absence and swept this attempt's processes;
    /// no consumer remains for its spec, sentinel or waypoint. Attempt every file
    /// so one obstruction cannot strand the other leftovers. Missing files are
    /// already clean, and every other failure joins the error recorded for retry.
    pub(super) async fn remove_failed_create_fail_closed(
        &self,
        mut error: anyhow::Error,
    ) -> anyhow::Error {
        if let Err(cleanup) =
            Self::remove_file_fail_closed(self.spec(), "the failed create's spec").await
        {
            error = error.context(cleanup);
        }
        if let Err(cleanup) =
            Self::remove_file_fail_closed(self.status(), "the failed create's sentinel").await
        {
            error = error.context(cleanup);
        }
        if let Err(cleanup) =
            Self::remove_file_fail_closed(self.waypoint(), "the failed create's waypoint").await
        {
            error = error.context(cleanup);
        }
        error
    }

    /// Remove a failed create's files after its process and tmux cleanup attempts.
    /// Keep the original spec → status → waypoint order and attempt every file;
    /// cleanup warnings must not replace the create failure being unwound.
    pub(super) async fn remove_failed_create_best_effort(&self) -> bool {
        let spec_removed = Self::remove_file_best_effort(self.spec(), "launch spec").await;
        let status_removed =
            Self::remove_file_best_effort(self.status(), "launch status file").await;
        let waypoint_removed =
            Self::remove_file_best_effort(self.waypoint(), "launch waypoint").await;
        spec_removed && status_removed && waypoint_removed
    }

    /// Retain every cleanup error when Delete has removed a relaunch's row.
    /// Spec → status → waypoint removals all run even if an earlier one fails;
    /// appending to the caller's teardown errors preserves their existing precedence.
    pub(super) async fn remove_deleted_relaunch_fail_closed(&self, errors: &mut Vec<String>) {
        if let Err(error) = Self::remove_file_fail_closed(self.spec(), "launch spec").await {
            errors.push(error);
        }
        if let Err(error) = Self::remove_file_fail_closed(self.status(), "launch sentinel").await {
            errors.push(error);
        }
        if let Err(error) = Self::remove_file_fail_closed(self.waypoint(), "launch waypoint").await
        {
            errors.push(error);
        }
    }

    /// Preserve the existing confirmed-absent restart cleanup before its sweep.
    /// Only spec and waypoint are removed, in that order; the sentinel remains.
    /// Both failures contextualize the existing launch error without skipping the
    /// caller's later process sweep or changing its final disposition.
    pub(super) async fn remove_unconsumed_relaunch_fail_closed(
        &self,
        mut error: anyhow::Error,
    ) -> anyhow::Error {
        if let Err(cleanup) =
            Self::remove_file_fail_closed(self.spec(), "the failed relaunch's spec").await
        {
            error = error.context(cleanup);
        }
        if let Err(cleanup) =
            Self::remove_file_fail_closed(self.waypoint(), "the failed relaunch's waypoint").await
        {
            error = error.context(cleanup);
        }
        error
    }

    /// Remove settled evidence without letting one failed unlink skip the rest.
    /// The caller has already preserved accepted-create evidence where necessary.
    /// Attempt every file so one obstruction does not retain the other leftovers;
    /// only complete success allows the observer to latch cleanup for this launch.
    async fn remove_settled_best_effort(&self) -> bool {
        let status_removed =
            Self::remove_file_best_effort(self.status(), "consumed launch sentinel").await;
        let spec_removed = Self::remove_file_best_effort(self.spec(), "leftover launch spec").await;
        let waypoint_removed =
            Self::remove_file_best_effort(self.waypoint(), "launch waypoint").await;
        status_removed && spec_removed && waypoint_removed
    }

    /// Clear reused paths before a retaken create can publish a new launch there.
    /// Sentinel first preserves the existing precedence: its survival can falsely
    /// classify the replacement. Every other removal failure also aborts the retry.
    async fn clear_for_retake_fail_closed(&self) -> Result<(), String> {
        Self::remove_file_fail_closed(self.status(), "the previous launch's sentinel").await?;
        Self::remove_file_fail_closed(self.waypoint(), "the previous launch's waypoint").await?;
        Self::remove_file_fail_closed(self.spec(), "the previous launch's spec").await
    }
}

/// Remove every launch spec, sentinel and waypoint belonging to `session_id`, across
/// all of its generations, fail-closed.
///
/// Delete's cleanup, and the one place that has to enumerate rather than
/// derive: a session that was restarted owns one artifact set per launch
/// (`launch::spec_path_for_launch`), and the row that would say how many is
/// about to be removed. Every failure — including a directory that cannot
/// be read at all — is returned rather than logged, because these files
/// hold agent command lines users put credentials into and delete is the
/// last moment anything will ever come back for them.
pub(crate) async fn remove_launch_artifacts_for_session(
    state_dir: &Path,
    session_id: &str,
) -> Result<(), String> {
    LaunchArtifacts::remove_for_session_fail_closed(state_dir, session_id).await
}

impl LaunchArtifacts {
    /// Delete exact owned directory entries, including staging copies, before the row.
    /// Enumeration order and first-error precedence remain the directory's own;
    /// deriving a canonical set would miss staging copies and change that contract.
    async fn remove_for_session_fail_closed(
        state_dir: &Path,
        session_id: &str,
    ) -> Result<(), String> {
        let launch_dir = state_dir.join("launch");
        let mut entries = match tokio::fs::read_dir(&launch_dir).await {
            Ok(entries) => entries,
            // A launch directory that was never created is nothing to clean up
            // — the honest empty case, unlike a directory this process cannot
            // read, which is reported.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => {
                return Err(format!(
                    "reading {} to remove this session's launch files: {e}",
                    launch_dir.display()
                ));
            }
        };
        loop {
            let entry = match entries.next_entry().await {
                Ok(None) => return Ok(()),
                Ok(Some(entry)) => entry,
                Err(e) => {
                    return Err(format!(
                        "listing {} to remove this session's launch files: {e}",
                        launch_dir.display()
                    ));
                }
            };
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if crate::launch::parse_launch_file_name(&name).is_some_and(|(id, _)| id == session_id)
                || staged_name_belongs_to(&name, session_id)
            {
                Self::remove_file_fail_closed(&entry.path(), "launch file").await?;
            }
        }
    }
}

/// Whether a staged temp name (`.<id>.<generation>.json.tmp-<uuid>`) stages
/// a launch SPEC rather than a status sentinel. See `sweep_launch_dir` for
/// why the two are treated differently at startup.
fn staged_spec_name(name: &str) -> bool {
    name.split_once(".tmp-")
        .is_some_and(|(stem, _)| stem.ends_with(".json"))
}

/// Whether a staged launch artifact belongs to `session_id`.
///
/// Staging prefixes the final launch name with a dot and appends `.tmp-<uuid>`;
/// a failed post-publication unlink can therefore leave a second,
/// credential-bearing copy beside the published file. Teardown, and the
/// startup sweep's handling of staged STATUS files, identify the owner before
/// deleting anything so a concurrent launch for a surviving session is not
/// disturbed. (Staged specs are removed at startup whoever owns them; see
/// `sweep_launch_dir`.) Invalid stems are
/// deliberately rejected as owners, leaving them eligible for the startup
/// sweep's existing orphan cleanup.
fn staged_name_belongs_to(name: &str, session_id: &str) -> bool {
    if !crate::files::is_staged_temp_name(name) || !name.starts_with('.') {
        return false;
    }
    let Some(stem) = name.strip_prefix('.') else {
        return false;
    };
    let Some(stem) = stem.split_once(".tmp-").map(|(stem, _)| stem) else {
        return false;
    };
    crate::launch::parse_launch_file_name(stem).is_some_and(|(id, _)| id == session_id)
}

impl LaunchArtifacts {
    /// Best-effort removal never masks the failure its caller is already unwinding.
    /// Absence is success; other failures warn and remain retryable by the observer.
    async fn remove_file_best_effort(path: &Path, what: &str) -> bool {
        match tokio::fs::remove_file(path).await {
            Ok(()) => true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => true,
            Err(e) => {
                warn!(path = %path.display(), error = %e, "could not remove {what}");
                false
            }
        }
    }
}

impl LaunchArtifacts {
    /// Fail-closed removal accepts absence but returns every other filesystem failure.
    /// The caller keeps the existing label so error text and precedence remain stable.
    async fn remove_file_fail_closed(path: &Path, what: &str) -> Result<(), String> {
        match tokio::fs::remove_file(path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("removing {what} ({}): {e}", path.display())),
        }
    }
}

/// Sweep `<state_dir>/launch/` at supervisor startup: remove orphaned
/// staged temp files and launch SPECS that no session in `sessions` still
/// owns. Called once from `Supervisor::serve`, after the exclusivity bind
/// (this process must be provably the state dir's one supervisor before
/// touching anything) and after the session map has been reloaded from
/// the store (this sweep needs it to answer "does anything still own
/// this spec").
///
/// Sentinels (`.status` files) of a session no longer on record are NEVER
/// touched here — PLAN_M3.md item 5's durability promise for them would be
/// worthless if a blanket startup sweep could erase the very evidence a
/// later classifier needs to read; that lifecycle belongs to the
/// classifier or an explicit delete. The one exception is a live
/// session's SUPERSEDED launch: sentinel reads address one exact
/// generation, so an earlier generation's sentinel can never be read
/// again and goes with its spec.
///
/// A spec's session id (the first component of its `<id>.<generation>`
/// stem — `launch::parse_launch_file_name`) is checked against `sessions` —
/// rather than removing every entry unconditionally, which is what this
/// sweep used to do — because a supervisor restart does NOT kill tmux: a
/// session created just before the restart can have its login shell
/// STILL mid-flight toward `exec farhelm internal launch <spec>`,
/// arbitrarily long after tmux itself created the window (a slow or hung
/// rc-file is a real, if rare, way this stretches out). Its session id is
/// already durably recorded (the just-reloaded `sessions` map reflects
/// SQLite, loaded before this sweep runs), so "does a session with this
/// id exist" is a real ownership question, not a guess: a spec whose id
/// is UNKNOWN can only have gotten here two ways — the create that wrote
/// it crashed before the DB insert ever committed (nothing will ever read
/// it), or its session was since deleted and `DeleteSession`'s own
/// removal of it already failed (logged there) — either way, nothing
/// alive will ever come back for it.
///
/// `sessions` maps each session still on record to its current launch
/// generation, so a live session's superseded specs are removed too.
///
/// Best-effort and log-only: this sweep is tidiness rather than a
/// credential boundary (a spec holds nothing the session's own database row
/// does not already hold for the session's lifetime; see SPEC_impl.md's
/// runtime-state notes), so a failure that leaves debris behind is logged
/// and never fails startup.
pub(crate) async fn sweep_launch_dir(
    launch_dir: &Path,
    sessions: &std::collections::HashMap<String, i64>,
) {
    LaunchArtifacts::sweep_dir(launch_dir, sessions).await;
}

impl LaunchArtifacts {
    /// Sweep selected exact entries after exclusive ownership and session reload.
    /// Selection preserves live launch evidence; orphan sentinels and surviving
    /// shims' staged sentinels cannot be removed as a whole generation's file set.
    async fn sweep_dir(launch_dir: &Path, sessions: &std::collections::HashMap<String, i64>) {
        let mut entries = match tokio::fs::read_dir(launch_dir).await {
            Ok(entries) => entries,
            Err(e) => {
                warn!(error = %e, "could not sweep launch dir; orphaned entries may remain");
                return;
            }
        };
        loop {
            let entry = match entries.next_entry().await {
                Ok(None) => break,
                Ok(Some(entry)) => entry,
                Err(e) => {
                    warn!(error = %e, "launch-dir sweep aborted early; orphaned entries may remain");
                    break;
                }
            };
            let name = entry.file_name();
            let name = name.to_string_lossy();

            let should_remove = if crate::files::is_staged_temp_name(&name) {
                // A restart leaves the old shim alive. Its unpublished staged
                // sentinel is still launch-failure evidence, so a staged
                // `.status` is orphaned only when its parsed owner is absent.
                //
                // A staged `.json` is different: only a supervisor writes specs,
                // and this sweep runs after this one proved it is the state
                // directory's sole owner and before it launches anything, so a
                // staged spec present now was left by a dead supervisor (a crash
                // between link and unlink, or mid-write) and can never be
                // finished or read. Keeping it only kept a hidden second copy of
                // the command line and session token until the session was
                // deleted, so it goes whoever owns it.
                staged_spec_name(&name)
                    || !sessions
                        .keys()
                        .any(|session_id| staged_name_belongs_to(&name, session_id))
            } else if let Some((id, generation)) = crate::launch::parse_launch_file_name(&name) {
                // Names are `<id>.<generation>.json|status|waypoint`: launch files are
                // per-LAUNCH (`launch::spec_path_for_launch`). Two rules:
                //
                // - A session no longer on record loses its spec and waypoint. Its
                //   sentinels stay: the classifier's durability promise (this
                //   function's docs) is about evidence whose owner is unknown.
                // - A live session loses every file of a SUPERSEDED launch, spec
                //   and sentinel alike. A restart writes the next generation and
                //   nothing reads an older one again: sentinel reads address one
                //   exact generation (`launch::read_launch_sentinel`), so an
                //   earlier launch's sentinel is unreachable, and its spec is a
                //   leftover no restart path removes (a login shell that died in
                //   its rc files never reached the shim that would have). The
                //   reloaded map says which generation is current, so this is a
                //   registry question, not a guess from the directory listing.
                //   The CURRENT launch's files are kept even unread: its shim may
                //   still be mid-flight.
                match sessions.get(id) {
                    None => name.ends_with(".json") || name.ends_with(".waypoint"),
                    Some(current) => generation < *current,
                }
            } else {
                false
            };

            // Preserve the sweep's existing warning for every unlink error, including
            // an entry disappearing after enumeration. Settlement cleanup instead
            // treats absence as success; changing that here would change its logs.
            if should_remove && let Err(e) = tokio::fs::remove_file(entry.path()).await {
                warn!(path = %entry.path().display(), error = %e,
                "could not remove orphaned launch-dir entry");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::core::tests::StateDir;
    use super::*;

    /// Item 1's regression, and the reason `sweep_launch_dir` exists at
    /// all instead of the old blanket "remove everything" sweep: a
    /// durable exec-failure sentinel must survive this sweep no matter
    /// what, even for a session no longer tracked (there is no session in
    /// this test at all) — only PR5's future classifier, or an explicit
    /// delete, may ever remove one. An orphaned staged file and spec (their
    /// session ids absent from `sessions`) are seeded alongside it and must
    /// both go, while a surviving session's unpublished staged sentinel
    /// remains for its still-running shim to publish. That proves the sweep
    /// classifies staging by ownership rather than skipping the directory.
    #[farhelm_testtrace::test]
    async fn sweep_launch_dir_never_removes_a_sentinel() {
        let tmp = tempfile::tempdir().unwrap();
        let launch_dir = tmp.path().join("launch");
        std::fs::create_dir(&launch_dir).unwrap();
        std::fs::write(
            launch_dir.join("abc.0.status"),
            b"exec_failed argv0=x errno=2",
        )
        .unwrap();
        std::fs::write(launch_dir.join("orphan.0.json"), b"{}").unwrap();
        // A live session's current generation must survive: its shell may
        // still be on its way to the shim that will consume this spec.
        std::fs::write(launch_dir.join("live.3.json"), b"{}").unwrap();
        std::fs::write(launch_dir.join(".orphan.0.json.tmp-deadbeef"), b"partial").unwrap();
        // A live session's staged SPEC: only a supervisor writes specs, so
        // at startup this one was left by a dead supervisor and is an
        // unreadable second copy of the session's command line and token.
        std::fs::write(launch_dir.join(".live.3.json.tmp-deadbeef"), b"credentials").unwrap();
        std::fs::write(
            launch_dir.join(".live.3.status.tmp-deadbeef"),
            b"unpublished exec failure",
        )
        .unwrap();
        // Unrecognized names are never this sweep's to remove.
        std::fs::write(launch_dir.join("not-ours"), b"?").unwrap();

        let live: std::collections::HashMap<String, i64> =
            [("live".to_string(), 3)].into_iter().collect();
        sweep_launch_dir(&launch_dir, &live).await;

        assert!(
            launch_dir.join("abc.0.status").exists(),
            "a sentinel must never be removed by this sweep, regardless of session ownership"
        );
        assert!(
            !launch_dir.join("orphan.0.json").exists(),
            "a spec whose session id owns nothing in `sessions` must be removed"
        );
        assert!(
            launch_dir.join("live.3.json").exists(),
            "a live session's spec survives, whichever generation named it"
        );
        assert!(
            !launch_dir.join(".orphan.0.json.tmp-deadbeef").exists(),
            "an orphaned staged temp file must be removed"
        );
        assert!(
            !launch_dir.join(".live.3.json.tmp-deadbeef").exists(),
            "a staged spec is a dead supervisor's debris even for a live session"
        );
        assert!(
            launch_dir.join(".live.3.status.tmp-deadbeef").exists(),
            "a surviving session's unpublished sentinel must remain for its shim to publish"
        );
        assert!(
            launch_dir.join("not-ours").exists(),
            "an unrecognized file is not this sweep's to delete"
        );
    }

    /// The name parser the sweep depends on, pinned directly: session ids
    /// are UUIDs (no dots), so splitting the last two dot-separated
    /// components apart is unambiguous — and anything that does not parse
    /// must come back `None` rather than being guessed at, since the sweep
    /// deletes what it recognizes.
    #[farhelm_testtrace::test]
    fn launch_file_names_round_trip_through_the_parser() {
        let state = std::path::Path::new("/state");
        let spec = crate::launch::spec_path_for_launch(state, "sess-1", 7);
        let status = crate::launch::status_path_for_spec(&spec);
        let waypoint = crate::launch::waypoint_path_for_spec(&spec);
        for path in [&spec, &status, &waypoint] {
            let name = path.file_name().unwrap().to_string_lossy();
            assert_eq!(
                crate::launch::parse_launch_file_name(&name),
                Some(("sess-1", 7)),
                "{name} must parse back into the launch that produced it"
            );
        }
        assert_eq!(crate::launch::parse_launch_file_name("sess-1.json"), None);
        assert_eq!(crate::launch::parse_launch_file_name("sess-1.x.json"), None);
        assert_eq!(crate::launch::parse_launch_file_name("tmux.conf"), None);
        assert_eq!(crate::launch::parse_launch_file_name(".0.json"), None);
    }

    /// A staged copy is safe for session teardown only when its de-dotted
    /// stem identifies that session. This protects another session's
    /// in-flight write and leaves unrelated temporary files for their owner.
    #[farhelm_testtrace::test]
    fn staged_launch_name_belongs_only_to_its_parsed_session() {
        assert!(staged_name_belongs_to(
            ".this-session.0.json.tmp-deadbeef",
            "this-session"
        ));
        assert!(staged_name_belongs_to(
            ".this-session.0.status.tmp-deadbeef",
            "this-session"
        ));
        assert!(!staged_name_belongs_to(
            "this-session.0.json.tmp-deadbeef",
            "this-session"
        ));
        assert!(!staged_name_belongs_to(
            ".not-a-launch-file.tmp-deadbeef",
            "not-a-launch-file"
        ));
    }

    /// Delete must remove an orphaned staged copy for the same
    /// session, while preserving another session's active write and a temp
    /// file whose stem is not a launch name.
    #[farhelm_testtrace::test]
    async fn remove_launch_artifacts_for_session_removes_its_staged_copy() {
        let tmp = tempfile::tempdir().unwrap();
        let launch_dir = tmp.path().join("launch");
        std::fs::create_dir(&launch_dir).unwrap();
        let session = "this-session";
        let other = "other-session";
        let own_spec = launch_dir.join(format!("{session}.0.json"));
        let own_staged = launch_dir.join(format!(".{session}.0.json.tmp-deadbeef"));
        let other_staged = launch_dir.join(format!(".{other}.0.json.tmp-deadbeef"));
        let own_waypoint = launch_dir.join(format!("{session}.0.waypoint"));
        let other_waypoint = launch_dir.join(format!("{other}.0.waypoint"));
        let unrelated = launch_dir.join(".not-a-launch-file.tmp-deadbeef");
        for path in [
            &own_spec,
            &own_staged,
            &other_staged,
            &own_waypoint,
            &other_waypoint,
            &unrelated,
        ] {
            std::fs::write(path, b"credential-bearing launch data").unwrap();
        }

        remove_launch_artifacts_for_session(tmp.path(), session)
            .await
            .unwrap();

        assert!(!own_spec.exists());
        assert!(!own_staged.exists());
        assert!(!own_waypoint.exists());
        assert!(other_staged.exists());
        assert!(other_waypoint.exists());
        assert!(unrelated.exists());
    }

    /// Spec: for a session still on record, the sweep keeps its CURRENT
    /// generation's artifacts and removes all three for earlier
    /// generations.
    ///
    /// Item 22's restart race is why the current spec must survive: a
    /// supervisor restart does not kill tmux, so the login shell behind that
    /// session can still be mid-flight toward reading it, arbitrarily long
    /// after the window was created. An earlier generation's spec is
    /// different: a restart already wrote its successor, nothing reads the
    /// old one again, and before this sweep learned generations it stayed on
    /// disk until Delete whenever its shell never reached the shim.
    #[farhelm_testtrace::test]
    async fn sweep_launch_dir_keeps_the_current_launch_and_removes_superseded_ones() {
        let tmp = tempfile::tempdir().unwrap();
        let launch_dir = tmp.path().join("launch");
        std::fs::create_dir(&launch_dir).unwrap();
        for name in [
            "live.1.json",
            "live.1.status",
            "live.1.waypoint",
            "live.2.json",
            "live.2.status",
            "live.2.waypoint",
            "orphan.0.waypoint",
        ] {
            std::fs::write(launch_dir.join(name), b"{}").unwrap();
        }

        let sessions = std::collections::HashMap::from([("live".to_string(), 2)]);
        sweep_launch_dir(&launch_dir, &sessions).await;

        assert!(
            launch_dir.join("live.2.json").exists(),
            "the current launch's spec must survive: its shim may still be mid-flight"
        );
        assert!(
            !launch_dir.join("live.1.json").exists(),
            "a superseded launch's spec is never read again and must go"
        );
        assert!(
            !launch_dir.join("live.1.status").exists(),
            "a superseded launch's sentinel can never be read again and must go"
        );
        assert!(
            launch_dir.join("live.2.status").exists(),
            "the current launch's sentinel is its classifier's evidence and must stay"
        );
        assert!(launch_dir.join("live.2.waypoint").exists());
        assert!(!launch_dir.join("live.1.waypoint").exists());
        assert!(!launch_dir.join("orphan.0.waypoint").exists());
    }

    /// Only a dead, owned pane and an unconsumed spec identify a never-started
    /// launch. This proof works without systemd; the compatibility bit and
    /// waypoint refine the failed step without changing the evidence threshold.
    #[farhelm_testtrace::test]
    async fn a_wrapper_failure_is_recognized_only_by_its_full_shape() {
        let state = StateDir::new();
        let id = uuid::Uuid::new_v4().to_string();
        let spec = crate::launch::spec_path_for_launch(state.path(), &id, 0);
        std::fs::create_dir_all(spec.parent().expect("launch dir")).expect("launch dir");

        assert_eq!(
            wrapper_failure_detail(state.path(), &id, 0, true, true, Some(1))
                .await
                .unwrap(),
            None,
            "no spec on disk means the shim consumed it and really did run"
        );

        std::fs::write(&spec, b"{}").expect("plant an unconsumed spec");
        assert!(
            wrapper_failure_detail(state.path(), &id, 0, false, true, Some(7))
                .await
                .unwrap()
                .is_some_and(|detail| detail.contains("your login shell exited (code 7)")),
            "an unscoped launch goes directly from the shell to the shim"
        );
        assert_eq!(
            wrapper_failure_detail(state.path(), &id, 0, true, false, None)
                .await
                .unwrap(),
            None,
            "an ABSENT pane means the window or the whole tmux server was destroyed, which \
             strands a spec from a launch that was interrupted rather than failed"
        );
        assert!(
            wrapper_failure_detail(state.path(), &id, 0, true, true, Some(1))
                .await
                .unwrap()
                .is_some_and(|detail| detail.contains("never reached farhelm's exec shim")),
            "a scoped launch whose spec was never consumed, under a dead pane, never started"
        );

        // Per LAUNCH, like every other artifact keyed on the generation: a
        // spec left by generation 0 must not paint generation 1 as failed.
        assert_eq!(
            wrapper_failure_detail(state.path(), &id, 1, true, true, Some(1))
                .await
                .unwrap(),
            None,
            "a previous generation's leftover spec is not this launch's evidence"
        );

        std::fs::write(&spec, br#"{"scope_waypoint_expected":true}"#).unwrap();
        let shell = wrapper_failure_detail(state.path(), &id, 0, true, true, Some(3))
            .await
            .unwrap()
            .unwrap();
        assert!(
            shell.contains("your login shell exited (code 3)"),
            "{shell}"
        );
        let waypoint = crate::launch::waypoint_path_for_spec(&spec);
        std::fs::write(&waypoint, b"").unwrap();
        let wrapper = wrapper_failure_detail(state.path(), &id, 0, true, true, Some(4))
            .await
            .unwrap()
            .unwrap();
        assert!(
            wrapper.contains("systemd scope wrapper exited (code 4)"),
            "{wrapper}"
        );
        let signal = wrapper_failure_detail(state.path(), &id, 0, true, true, None)
            .await
            .unwrap()
            .unwrap();
        assert!(
            !signal.contains("(code"),
            "signal deaths have no invented code: {signal}"
        );

        // A scope selected durably may lose its manager before argv assembly.
        // An explicitly false bit describes that new, unwrapped command; only
        // an absent bit belongs to the ambiguous pre-waypoint format.
        std::fs::write(&spec, br#"{"scope_waypoint_expected":false}"#).unwrap();
        let fallback = wrapper_failure_detail(state.path(), &id, 0, true, true, Some(6))
            .await
            .unwrap()
            .unwrap();
        assert!(
            fallback.contains("your login shell exited (code 6)"),
            "{fallback}"
        );
        std::fs::write(&spec, br#"{"scope_waypoint_expected":true}"#).unwrap();

        std::fs::remove_file(&waypoint).unwrap();
        std::os::unix::fs::symlink(&waypoint, &waypoint).unwrap();
        assert!(
            wrapper_failure_detail(state.path(), &id, 0, true, true, None)
                .await
                .is_err(),
            "a waypoint stat failure cannot prove the shell never reached it"
        );
        std::fs::remove_file(&waypoint).unwrap();

        // An unreadable spec is inconclusive, never evidence that it was consumed.
        std::fs::remove_file(&spec).unwrap();
        std::fs::create_dir(&spec).unwrap();
        assert!(
            wrapper_failure_detail(state.path(), &id, 0, true, true, None)
                .await
                .is_err()
        );
    }

    /// A retaken generation zero must not inherit its predecessor's waypoint.
    /// Failing to remove it would blame a new shell failure on the old wrapper.
    #[farhelm_testtrace::test]
    async fn retaking_a_launch_clears_its_waypoint_fail_closed() {
        let state = StateDir::new();
        let spec = crate::launch::spec_path_for_launch(state.path(), "retaken", 0);
        std::fs::create_dir_all(spec.parent().unwrap()).unwrap();
        let waypoint = crate::launch::waypoint_path_for_spec(&spec);
        std::fs::create_dir(&waypoint).unwrap();
        assert!(
            clear_launch_artifacts_fail_closed(state.path(), "retaken", 0)
                .await
                .is_err()
        );
        std::fs::remove_dir(&waypoint).unwrap();
        std::fs::write(&waypoint, b"").unwrap();
        std::fs::write(&spec, b"{}").unwrap();
        clear_launch_artifacts_fail_closed(state.path(), "retaken", 0)
            .await
            .unwrap();
        assert!(!spec.exists() && !waypoint.exists());
    }

    /// Waypoints become unnecessary once Error is durable, and cleanup must
    /// remove them alongside the spec and sentinel before latching success.
    #[farhelm_testtrace::test]
    async fn settled_launch_cleanup_removes_all_three_artifacts() {
        let state = StateDir::new();
        let store = SessionStore::open(&state.path().join("cleanup.db"), true)
            .await
            .unwrap();
        let spec = crate::launch::spec_path_for_launch(state.path(), "settled", 1);
        std::fs::create_dir_all(spec.parent().unwrap()).unwrap();
        let status = crate::launch::status_path_for_spec(&spec);
        let waypoint = crate::launch::waypoint_path_for_spec(&spec);
        for path in [&spec, &status, &waypoint] {
            std::fs::write(path, b"fixture").unwrap();
        }
        assert!(cleanup_launch_artifacts(state.path(), &store, "settled", 1).await);
        assert!(!spec.exists() && !status.exists() && !waypoint.exists());
    }

    /// A sentinel obstruction must keep cleanup retryable without retaining the
    /// other files. This pins attempt-all behavior independently of store settlement,
    /// so replacing the policy with a short-circuiting loop cannot pass unnoticed.
    #[farhelm_testtrace::test]
    async fn settled_cleanup_attempts_every_file_after_an_early_removal_failure() {
        let state = StateDir::new();
        let artifacts = LaunchArtifacts::for_launch(state.path(), "obstructed", 1);
        std::fs::create_dir_all(artifacts.spec().parent().unwrap()).unwrap();
        std::fs::create_dir(artifacts.status()).unwrap();
        std::fs::write(artifacts.spec(), b"credential-bearing fixture").unwrap();
        std::fs::write(artifacts.waypoint(), b"").unwrap();
        assert!(artifacts.status().is_dir());
        assert!(artifacts.spec().is_file() && artifacts.waypoint().is_file());

        assert!(!artifacts.remove_settled_best_effort().await);
        assert!(artifacts.status().is_dir());
        assert!(!artifacts.spec().exists() && !artifacts.waypoint().exists());

        // A later pass can confirm cleanup after the obstruction disappears;
        // already-removed siblings are ordinary success, not a new failure.
        std::fs::remove_dir(artifacts.status()).unwrap();
        assert!(artifacts.remove_settled_best_effort().await);
    }

    /// A retake must stop at the first obstruction in sentinel → waypoint → spec
    /// order, because it cannot publish over evidence it failed to remove. Directory
    /// obstructions exercise real unlink failures without permission or uid assumptions.
    #[farhelm_testtrace::test]
    async fn retake_cleanup_preserves_order_and_stops_at_the_first_removal_failure() {
        let state = StateDir::new();
        let artifacts = LaunchArtifacts::for_launch(state.path(), "retake-order", 0);
        std::fs::create_dir_all(artifacts.spec().parent().unwrap()).unwrap();
        std::fs::create_dir(artifacts.status()).unwrap();
        std::fs::write(artifacts.spec(), b"credential-bearing fixture").unwrap();
        std::fs::write(artifacts.waypoint(), b"").unwrap();
        assert!(artifacts.status().is_dir());
        assert!(artifacts.spec().is_file() && artifacts.waypoint().is_file());

        let error = artifacts.clear_for_retake_fail_closed().await.unwrap_err();
        assert!(error.starts_with("removing the previous launch's sentinel ("));
        assert!(error.contains(&artifacts.status().display().to_string()));
        assert!(artifacts.spec().is_file() && artifacts.waypoint().is_file());

        std::fs::remove_dir(artifacts.status()).unwrap();
        std::fs::write(artifacts.status(), b"failure fixture").unwrap();
        std::fs::remove_file(artifacts.waypoint()).unwrap();
        std::fs::create_dir(artifacts.waypoint()).unwrap();
        assert!(artifacts.status().is_file());
        assert!(artifacts.waypoint().is_dir() && artifacts.spec().is_file());

        let error = artifacts.clear_for_retake_fail_closed().await.unwrap_err();
        assert!(error.starts_with("removing the previous launch's waypoint ("));
        assert!(error.contains(&artifacts.waypoint().display().to_string()));
        assert!(!artifacts.status().exists());
        assert!(artifacts.spec().is_file());

        std::fs::remove_dir(artifacts.waypoint()).unwrap();
        artifacts.clear_for_retake_fail_closed().await.unwrap();
        assert!(!artifacts.spec().exists());
        artifacts.clear_for_retake_fail_closed().await.unwrap();
    }

    /// A relaunch that lost its row to Delete must report every remaining artifact
    /// failure alongside earlier teardown failures. This differs from a retake's
    /// short circuit: later files must still be removed after a spec obstruction.
    #[farhelm_testtrace::test]
    async fn deleted_relaunch_cleanup_aggregates_failures_and_attempts_later_files() {
        let state = StateDir::new();
        let artifacts = LaunchArtifacts::for_launch(state.path(), "deleted-relaunch", 1);
        std::fs::create_dir_all(artifacts.spec().parent().unwrap()).unwrap();
        for path in [artifacts.spec(), artifacts.status(), artifacts.waypoint()] {
            std::fs::create_dir(path).unwrap();
            assert!(path.is_dir());
        }
        let mut errors = vec!["earlier teardown failure".to_string()];

        artifacts
            .remove_deleted_relaunch_fail_closed(&mut errors)
            .await;

        assert_eq!(errors.len(), 4);
        assert_eq!(errors[0], "earlier teardown failure");
        for (error, label, path) in [
            (&errors[1], "launch spec", artifacts.spec()),
            (&errors[2], "launch sentinel", artifacts.status()),
            (&errors[3], "launch waypoint", artifacts.waypoint()),
        ] {
            assert!(error.starts_with(&format!("removing {label} (")));
            assert!(error.contains(&path.display().to_string()));
            assert!(path.is_dir());
        }

        std::fs::remove_dir(artifacts.status()).unwrap();
        std::fs::remove_dir(artifacts.waypoint()).unwrap();
        std::fs::write(artifacts.status(), b"failure fixture").unwrap();
        std::fs::write(artifacts.waypoint(), b"").unwrap();
        assert!(artifacts.spec().is_dir());
        assert!(artifacts.status().is_file() && artifacts.waypoint().is_file());
        errors.clear();

        artifacts
            .remove_deleted_relaunch_fail_closed(&mut errors)
            .await;

        assert_eq!(errors.len(), 1);
        assert!(errors[0].starts_with("removing launch spec ("));
        assert!(!artifacts.status().exists() && !artifacts.waypoint().exists());
    }

    /// Confirmed-absent restart cleanup retains the sentinel and attempts waypoint
    /// removal even when its spec cannot be removed. Its two cleanup contexts keep
    /// their original nesting, so the outer error still names the last failed step.
    #[farhelm_testtrace::test]
    async fn unconsumed_relaunch_cleanup_keeps_sentinel_and_error_context_order() {
        let state = StateDir::new();
        let artifacts = LaunchArtifacts::for_launch(state.path(), "absent-relaunch", 1);
        std::fs::create_dir_all(artifacts.spec().parent().unwrap()).unwrap();
        std::fs::create_dir(artifacts.spec()).unwrap();
        std::fs::write(artifacts.status(), b"retained sentinel").unwrap();
        std::fs::write(artifacts.waypoint(), b"").unwrap();
        assert!(artifacts.spec().is_dir());
        assert!(artifacts.status().is_file() && artifacts.waypoint().is_file());

        let error = artifacts
            .remove_unconsumed_relaunch_fail_closed(anyhow::anyhow!("original tmux failure"))
            .await;

        assert!(!artifacts.waypoint().exists());
        assert_eq!(
            std::fs::read(artifacts.status()).unwrap(),
            b"retained sentinel"
        );
        let chain: Vec<_> = error.chain().map(ToString::to_string).collect();
        assert_eq!(chain.len(), 2);
        assert!(chain[0].starts_with("removing the failed relaunch's spec ("));
        assert_eq!(chain[1], "original tmux failure");

        std::fs::create_dir(artifacts.waypoint()).unwrap();
        assert!(artifacts.spec().is_dir() && artifacts.waypoint().is_dir());
        assert!(artifacts.status().is_file());

        let error = artifacts
            .remove_unconsumed_relaunch_fail_closed(anyhow::anyhow!("original tmux failure"))
            .await;

        let chain: Vec<_> = error.chain().map(ToString::to_string).collect();
        assert_eq!(chain.len(), 3);
        assert!(chain[0].starts_with("removing the failed relaunch's waypoint ("));
        assert!(chain[0].contains(&artifacts.waypoint().display().to_string()));
        assert!(chain[1].starts_with("removing the failed relaunch's spec ("));
        assert!(chain[1].contains(&artifacts.spec().display().to_string()));
        assert_eq!(chain[2], "original tmux failure");
        assert_eq!(
            std::fs::read(artifacts.status()).unwrap(),
            b"retained sentinel"
        );
    }
}
