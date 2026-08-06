//! Alt-screen snapshot capture, publication, and startup cleanup.
//!
//! A `StopSession` on a pane running a full-screen (alternate-buffer) TUI
//! attempts to capture a bounded last frame so a later `Attach` has
//! something to show (an oversized frame or a failed tmux read yields
//! none, deliberately); see
//! `capture_alt_screen_before_stop` and `publish_alt_screen_snapshot` for
//! the two-phase capture-then-publish protocol and why the ordering
//! between them is load-bearing. Unrelated to `SessionEntry`'s own
//! `snapshot` field (`IntegrationSnapshot`, conversation-capture
//! identity, in `service::core`) — the two concepts share only the
//! English word.

use super::core::Supervisor;
use super::terminals::Terminal;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tracing::warn;

/// Upper bound on an alt-screen snapshot, applied at BOTH ends of its
/// life: how much of a live `capture-pane` invocation's output
/// [`TmuxDriver::capture_alt_screen_if_active`]'s bounded reader will
/// buffer before killing the child and discarding, and how much of a
/// STORED snapshot file [`read_bounded_snapshot_file`] will read before
/// giving up and degrading an attach to the plain prefill. Both bounds
/// exist for the same underlying reason and share this one constant so
/// they can never silently drift apart.
///
/// A hostile or merely huge pane (an agent running at an enormous
/// terminal size, deliberately or not) must not turn `stop` — a rare but
/// latency-sensitive operation callers are waiting on — into an unbounded
/// in-memory buffer and an unbounded private-file write; nor should a
/// corrupted or tampered-with snapshot FILE be able to make an ordinary
/// `Attach` read an unbounded amount off disk. 2 MiB is generous for a
/// single screen's worth of styled cells (SPEC.md's own replay floor,
/// [`HISTORY_LIMIT`](crate::tmux::HISTORY_LIMIT), budgets for 12,000
/// LINES of full scrollback; this cap covers a single frame) while still
/// bounding the worst case. An over-cap capture or read is dropped with a
/// warning, exactly like any other best-effort snapshot failure.
pub(crate) const MAX_ALT_SCREEN_SNAPSHOT_BYTES: usize = 2 * 1024 * 1024;

/// Path to a session's alt-screen stop snapshot (whether or not it
/// currently exists).
///
/// A STABLE path keyed by the session id — not a fresh, one-time name
/// like the per-write files a versioned log would use — because the file
/// is deliberately REPLACED by every later stop rather than accumulated,
/// since only the most recent screen before a kill is worth keeping (see
/// [`capture_alt_screen_before_stop`] / [`publish_alt_screen_snapshot`]).
/// Same confidentiality class as a launch spec — terminal content can
/// carry secrets an agent echoed — hence living under its own
/// `ensure_private_dir`-protected subdirectory rather than next to the
/// launch specs themselves.
///
/// Restart interplay (no extra code needed beyond this path being keyed
/// by session id, which is stable across a restart): snapshot files
/// persist across supervisor restarts on the same state dir, so a
/// reloaded session whose tmux pane survived the restart can still hit
/// the `Attach` handler's dead-pane-replay path later using a snapshot
/// from a stop that predates the restart. A terminal-less (restart-gap)
/// session's snapshot, if any, is simply unreachable — `Attach` refuses a
/// terminal-less entry before ever consulting a snapshot — until
/// `DeleteSession` cleans it up.
pub(crate) fn snapshot_path(state_dir: &Path, session_id: &str) -> PathBuf {
    state_dir.join("snapshots").join(session_id)
}

/// Sweep abandoned `overwrite_private_file` staging files (`*.tmp-*`)
/// out of `<state_dir>/snapshots/` at supervisor startup — called once
/// from `Supervisor::serve`, same spirit and placement as the launch-dir
/// sweep just above it (after the exclusivity bind, so this process is
/// provably the state dir's one supervisor before touching anything).
///
/// `overwrite_private_file` already cleans up its own temp file when its
/// write or rename fails (`crate::files::remove_temp_after_failure`), but
/// that cleanup only runs if THIS process is still alive to run it — a
/// hard crash (OOM kill, `kill -9`, power loss) between staging the temp
/// file and either renaming it into place or reaching the failure-cleanup
/// path skips it entirely, leaving an orphaned `.tmp-*` file behind
/// forever with nothing else that would ever remove it. This sweep is
/// that backstop.
///
/// Deliberately narrower than a blanket sweep: `snapshots/` also holds
/// legitimate, PERSISTENT snapshot files meant to survive a restart (see
/// `snapshot_path`'s "restart interplay" docs), so this sweep only ever
/// removes entries matching [`crate::files::is_staged_temp_name`] — the
/// SAME naming convention every write-atomicity tier's temp file shares,
/// so this one pattern covers debris from `crate::files`'s helpers
/// regardless of which tier staged it. A real snapshot, named after a
/// session id alone, can never match that pattern (a session id contains
/// no `.tmp-` substring by construction: it is a UUID's hyphenated hex
/// form).
///
/// Best-effort and log-only, like the launch-dir sweep: an absent
/// `snapshots/` directory (no supervisor on this state dir has ever
/// captured a snapshot yet) is the ordinary case and not worth a log
/// line; any other read/remove failure is warned about but never fails
/// startup — a leftover temp file is debris, not a correctness problem
/// for anything this sweep's caller is trying to do.
pub(crate) async fn sweep_snapshot_temp_files(state_dir: &Path) {
    let snapshots_dir = state_dir.join("snapshots");
    let mut entries = match tokio::fs::read_dir(&snapshots_dir).await {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
        Err(e) => {
            warn!(error = %e, "could not sweep snapshot temp files; orphaned staging files may remain");
            return;
        }
    };
    loop {
        match entries.next_entry().await {
            Ok(None) => break,
            Ok(Some(entry)) => {
                let is_temp_file = entry
                    .file_name()
                    .to_str()
                    .is_some_and(crate::files::is_staged_temp_name);
                if is_temp_file && let Err(e) = tokio::fs::remove_file(entry.path()).await {
                    warn!(path = %entry.path().display(), error = %e,
                        "could not remove orphaned snapshot temp file");
                }
            }
            Err(e) => {
                warn!(error = %e,
                    "snapshot temp-file sweep aborted early; orphaned staging files may remain");
                break;
            }
        }
    }
}

/// Capture an alt-screen pane's visible content just before
/// [`kill_process_tree`] destroys it — WITHOUT writing anything to disk
/// yet. See [`publish_alt_screen_snapshot`] for why the write is a
/// separate, later step gated on the kill's own outcome.
///
/// Returns `None` whenever there is nothing worth storing: the pane was
/// not actually on the alternate screen at capture time (checked
/// ATOMICALLY with the capture itself, in the same tmux invocation — see
/// [`TmuxDriver::capture_alt_screen_if_active`]'s docs for the race two
/// separate calls would open, and for why that same call ALSO enforces
/// [`MAX_ALT_SCREEN_SNAPSHOT_BYTES`] itself via a bounded reader rather
/// than this function checking a length after the fact), a stale/recycled
/// pane id no longer belongs to this session, or the tmux call itself
/// failed. Every one of those is logged (except the unremarkable "was on
/// the primary screen" case) and swallowed here: the caller must still
/// proceed to `kill_process_tree` either way, and a lost snapshot is a
/// visibility regression, not a reason to fail the stop the caller
/// actually needs.
pub(crate) async fn capture_alt_screen_before_stop(
    sup: &Supervisor,
    session_id: &str,
    terminal: &Terminal,
) -> Option<Vec<u8>> {
    let capture = match sup
        .tmux
        .capture_alt_screen_if_active(
            &terminal.tmux_name,
            &terminal.pane,
            MAX_ALT_SCREEN_SNAPSHOT_BYTES,
        )
        .await
    {
        Ok(capture) => capture,
        Err(e) => {
            warn!(
                session = %session_id, error = %e,
                "capturing alt-screen snapshot before stop failed; reattach after stop will \
                 show a blank screen instead of the app's last frame"
            );
            return None;
        }
    };
    match capture {
        crate::tmux::AltScreenCapture::Captured(bytes) => Some(bytes),
        crate::tmux::AltScreenCapture::NotAlternate => None,
        crate::tmux::AltScreenCapture::SessionMismatch => {
            warn!(
                session = %session_id,
                "alt-screen capture's pane no longer belongs to this session (a stale pane id \
                 after a tmux server restart); skipping the snapshot"
            );
            None
        }
        crate::tmux::AltScreenCapture::TooLarge => {
            warn!(
                session = %session_id, cap = MAX_ALT_SCREEN_SNAPSHOT_BYTES,
                "alt-screen snapshot exceeds the size cap; skipping"
            );
            None
        }
    }
}

/// Write a snapshot [`capture_alt_screen_before_stop`] already captured,
/// guarded against a `DeleteSession` that raced the stop which produced
/// it.
///
/// Called ONLY after `kill_process_tree` has returned `Ok` — see the
/// `StopSession` call site. A stop that fails to kill must never publish:
/// without that ordering, a LATER natural exit's own dead-pane replay
/// could show "last screen before stop" for a stop that never actually
/// completed.
///
/// # Delete-race analysis
///
/// This file carries the same secrets-an-agent-echoed confidentiality
/// class as a launch spec, so writing one for a session that a concurrent
/// `DeleteSession` has ALREADY finished tearing down would orphan it
/// forever — nothing would ever come back to remove it. The fix is to
/// check `sup.sessions` for the session's continued existence BEFORE
/// writing anything, not after: `DeleteSession` removes a session's
/// snapshot file (`remove_fail_closed`) and then, still under the SAME
/// `attachments` lock, removes the session from `sup.sessions` itself
/// (see that handler's teardown block and the `Supervisor` struct's
/// lock-ordering docs). This function acquires that identical lock across
/// its own existence-check-then-write, which makes the two operations
/// mutually exclusive rather than merely racily ordered:
/// - If this function's lock acquisition wins, a concurrent delete cannot
///   even START its teardown until this function releases `attachments`
///   (it needs the same lock) — so the existence check below is
///   guaranteed accurate for the ENTIRE write that follows it, and the
///   delete that runs afterward will find (and fail-closed-remove) the
///   file this function just wrote, like any other artifact.
/// - If a concurrent delete's lock acquisition wins instead, its entire
///   teardown — snapshot removal AND the session's removal from
///   `sup.sessions` — completes before this function ever gets the lock.
///   The existence check then correctly finds the session gone and skips
///   the write entirely: there is nothing to clean up, because nothing
///   was ever written.
///
/// This is strictly simpler than a write-then-recheck-and-clean-up-if-
/// orphaned design (an earlier version of this function did exactly
/// that): checking first means an already-deleted session is a fast,
/// side-effect-free no-op, rather than a write immediately followed by
/// its own removal. (Full per-session lifecycle serialization — a lock
/// scoped to one session's whole stop/delete/attach lifecycle — was
/// considered and deliberately not built for this: reusing the existing
/// coarse `attachments` lock for this one short critical section is
/// enough to close the race without a new locking primitive.)
///
/// # Cancellation safety (the other half of the same race)
///
/// The `attachments`-lock analysis above assumes this function's own task
/// runs to completion. That is NOT guaranteed: `handle_connection`'s
/// shutdown tail (`HANDLER_SHUTDOWN_TIMEOUT`) can `abort()` whatever
/// `JoinSet`-tracked task is calling this — the `StopSession` handler —
/// mid-flight. An aborted task's local `attachments` `MutexGuard` is
/// dropped the moment cancellation unwinds its stack, even while it was
/// still `.await`ing a write; if that write were a plain
/// `spawn_blocking`-based one, the DETACHED blocking closure it kicked off
/// keeps running to completion regardless (blocking tasks are not
/// cancelled by dropping their `JoinHandle`) — so the rename that
/// publishes the snapshot can complete AFTER a concurrent `DeleteSession`,
/// unblocked by the just-released lock, has already found no file to
/// remove and finished tearing the session down entirely. The result: an
/// orphaned, secret-bearing snapshot file for a session the system
/// considers completely gone, which nothing will ever clean up.
///
/// The fix is to run the whole lock-acquire-check-write critical section
/// inside its OWN `tokio::spawn`'d task, entirely independent of whatever
/// task calls this function. Awaiting that inner task's `JoinHandle` is
/// itself cancellable — if THIS function's caller gets aborted while
/// waiting, only that await is cut short; the inner task keeps running to
/// natural completion exactly as if nothing happened, because nothing
/// besides its own (never-aborted) `JoinHandle` can cancel it. The
/// `attachments` lock is therefore held for the write's ENTIRE real
/// duration no matter what happens to this function's caller.
///
/// `seam` is a value, not a `&dyn` reference, and must be `Copy + Send +
/// 'static` so it can be moved into both the detached outer task and the
/// `spawn_blocking` closure the actual write runs inside — see
/// `crate::files::FaultSeam`'s own docs for why nothing in this crate
/// otherwise needs a seam to survive a thread hop. Production calls this
/// with [`crate::files::RealFs`] (see the `StopSession` call site); tests
/// can inject a failure through this exact function.
pub(crate) async fn publish_alt_screen_snapshot<S>(
    sup: &Arc<Supervisor>,
    session_id: &str,
    bytes: &[u8],
    seam: S,
) where
    S: crate::files::FaultSeam + Copy + Send + 'static,
{
    let dir = sup.state_dir.join("snapshots");
    if let Err(e) = crate::ensure_private_dir(&dir).await {
        warn!(session = %session_id, error = %e, "creating the snapshots directory failed");
        return;
    }
    let path = snapshot_path(&sup.state_dir, session_id);

    let sup = Arc::clone(sup);
    let session_id = session_id.to_string();
    let bytes = bytes.to_vec();
    let inner = tokio::spawn(async move {
        let attachments = sup.attachments.lock().await;
        let still_exists = sup.sessions.lock().await.contains_key(&session_id);
        if !still_exists {
            // A concurrent delete already finished (see the delete-race
            // analysis above): nothing to write, and — because nothing
            // was ever written — nothing to clean up either.
            drop(attachments);
            return;
        }
        let write_result = tokio::task::spawn_blocking(move || {
            crate::files::overwrite_private_file_sync(&path, &bytes, &seam)
        })
        .await;
        match write_result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                warn!(session = %session_id, error = %e, "writing the alt-screen snapshot failed");
            }
            Err(join_err) => {
                warn!(session = %session_id, error = %join_err,
                    "alt-screen snapshot write task panicked");
            }
        }
        drop(attachments);
    });
    // If THIS await is cancelled, `inner` is entirely unaffected — that
    // is the whole point (see the cancellation-safety docs above).
    if let Err(join_err) = inner.await {
        warn!(error = %join_err, "alt-screen snapshot publish task panicked");
    }
}

/// Read a stored alt-screen snapshot file, bounded the same way capture
/// time is (see [`MAX_ALT_SCREEN_SNAPSHOT_BYTES`]'s docs): reads at most
/// `cap + 1` bytes via [`AsyncReadExt::take`], so a corrupt, tampered-
/// with, or simply mis-sized file on disk can never be read into memory
/// unbounded — the same discipline
/// [`TmuxDriver::capture_alt_screen_if_active`]'s bounded reader already
/// applies on the write side. `Ok(None)` means the file does not exist,
/// the ordinary case for any session that either was never stopped on
/// the alternate screen or has since had its snapshot cleaned up by a
/// delete. An over-cap file (reading successfully hits `cap + 1`, the
/// smallest length a bounded reader can produce that proves there was
/// more) is reported as an `Err`, identical in shape to any other read
/// failure — see [`within_snapshot_cap`](crate::tmux::within_snapshot_cap)
/// for the shared at-cap-vs-one-over boundary this and the capture-side
/// reader both use.
async fn read_bounded_snapshot_file(path: &Path, cap: usize) -> std::io::Result<Option<Vec<u8>>> {
    let file = match tokio::fs::File::open(path).await {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let mut buf = Vec::new();
    file.take(cap as u64 + 1).read_to_end(&mut buf).await?;
    if !crate::tmux::within_snapshot_cap(buf.len(), cap) {
        return Err(std::io::Error::other(format!(
            "snapshot file at {} exceeds the {cap}-byte cap",
            path.display()
        )));
    }
    Ok(Some(buf))
}

/// Read back the alt-screen stop snapshot a replay should append for a
/// DEAD pane, or `None` when there is nothing to append.
///
/// Only READS it; the framing and sending live in
/// `Forwarder::send_dead_pane_snapshot`, which owns the bounded writer
/// queue this milestone introduced. The split exists so the snapshot's
/// two SOURCES (file, then pending map) stay one decision while the
/// send-side chunking follows the same stall-aware path as every other
/// byte a forwarder writes.
///
/// The gate is snapshot EXISTENCE, not the pane's current screen —
/// deliberately corrected from an earlier version of this function that
/// also required `!alternate_on`, reasoning that a dead pane still on the
/// alternate screen would already show its last frame via the ordinary
/// prefill above. That reasoning was empirically wrong: tmux replaces a
/// DEAD pane's own content — alternate screen or not, history or not —
/// with its own "Pane is dead" placeholder the moment the backing process
/// exits, so a dead-and-still-alternate pane's prefill shows nothing
/// useful either. That state is very much reachable in exactly the case
/// this feature exists for: a pane running an app that ignores SIGTERM,
/// which `StopSession`'s `kill_process_tree` escalates all the way to
/// SIGKILL — captured while alive and on the alternate screen, then
/// killed without ever getting a chance to restore the primary screen.
/// Gating on `!alternate_on` would blank exactly that case.
///
/// Consults the FILE first, then [`Supervisor::pending_snapshots`] —
/// see that field's own docs for the "attach lands between kill and
/// publish" window this fallback closes, and for the honesty argument
/// (why serving an in-flight capture is never showing stale or
/// misleading content). Both sources missing is the ordinary case for
/// most sessions (never stopped at all, or already cleaned up by a
/// delete) and is not logged; any actual read failure — on the file, not
/// its mere absence — degrades to the plain prefill with a warning rather
/// than failing the whole attach over a best-effort visibility extra.
pub(crate) async fn load_alt_screen_snapshot(
    sup: &Supervisor,
    session_id: &str,
) -> Option<Vec<u8>> {
    match read_bounded_snapshot_file(
        &snapshot_path(&sup.state_dir, session_id),
        MAX_ALT_SCREEN_SNAPSHOT_BYTES,
    )
    .await
    {
        Ok(Some(bytes)) => Some(bytes),
        Ok(None) => sup.pending_snapshots.lock().await.get(session_id).cloned(),
        Err(e) => {
            warn!(
                session = %session_id, error = %e,
                "reading the alt-screen snapshot failed; degrading to the plain prefill"
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::core::tests::{StateDir, dummy_exe};
    use super::super::core::{CaptureState, FirstInput, SessionEntry};
    use super::*;
    use crate::agent_kind::IntegrationSnapshot;
    use crate::store::LastOutcome;
    use farhelm_proto::{AgentKind, RestartOffer, SessionInfo, SessionStatus};

    /// Item 8: the alt-screen snapshot write must be injectable through
    /// its REAL production call site — `publish_alt_screen_snapshot`
    /// itself, called directly against a real `Supervisor` (constructed
    /// the same lightweight way `create_session_over_field_cap_...`
    /// does), not a synthetic call into `crate::files`. A seam that fails
    /// the write step must leave no snapshot file behind at all.
    #[tokio::test]
    async fn publish_alt_screen_snapshot_surfaces_an_injected_write_failure() {
        #[derive(Clone, Copy)]
        struct FailWrite;
        impl crate::files::FaultSeam for FailWrite {
            fn write(&self, _file: &mut std::fs::File, _bytes: &[u8]) -> std::io::Result<()> {
                Err(std::io::Error::other("injected snapshot write failure"))
            }
        }

        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");
        let session_id = "test-session".to_string();
        sup.sessions.lock().await.insert(
            session_id.clone(),
            Arc::new(SessionEntry {
                info: SessionInfo {
                    id: session_id.clone(),
                    title: "t".to_string(),
                    created_at: 1_700_000_000,
                    cwd: "/".to_string(),
                    invocation: "agent".to_string(),
                    status: SessionStatus::Unknown,
                    annotation: None,
                    restart_offer: RestartOffer::default(),
                    tabs: Vec::new(),
                    source_profile: None,
                },
                terminal: None,
                outcome: Arc::new(std::sync::Mutex::new(LastOutcome::Running)),
                snapshot: IntegrationSnapshot {
                    kind: AgentKind::Generic,
                    resume_template: None,
                },
                canonical_cwd: None,
                first_input: Arc::new(std::sync::Mutex::new(FirstInput {
                    at: None,
                    durable: true,
                })),
                capture: Arc::new(std::sync::Mutex::new(CaptureState::Unclaimed)),
                activity: crate::service::ticker::ActivitySample::unsampled(),
                generation: 0,
                scope: None,
            }),
        );

        publish_alt_screen_snapshot(&sup, &session_id, b"frame bytes", FailWrite).await;

        assert!(
            !snapshot_path(&sup.state_dir, &session_id).exists(),
            "an injected write failure must never publish a partial snapshot"
        );
    }

    /// Item 1's snapshot-directory counterpart to the launch-dir and
    /// tmux-config sweeps (tested in `launch_artifacts` and `core`
    /// respectively): a planted `.tmp-*` orphan in
    /// `snapshots/` must be removed while a REAL, persistent snapshot
    /// (named after its session id alone, per `snapshot_path`) survives
    /// untouched — proving the sweep recognizes only the shared
    /// `is_staged_temp_name` pattern and never a session id, however that
    /// id happens to be formatted.
    #[tokio::test]
    async fn sweep_snapshot_temp_files_removes_only_the_orphan() {
        let tmp = tempfile::tempdir().unwrap();
        let snapshots_dir = tmp.path().join("snapshots");
        std::fs::create_dir(&snapshots_dir).unwrap();
        let session_id = uuid::Uuid::new_v4().to_string();
        std::fs::write(snapshots_dir.join(&session_id), b"a real captured frame").unwrap();
        std::fs::write(
            snapshots_dir.join(format!(".{session_id}.tmp-deadbeef")),
            b"partial",
        )
        .unwrap();

        sweep_snapshot_temp_files(tmp.path()).await;

        assert!(
            snapshots_dir.join(&session_id).exists(),
            "a real, persistent snapshot must never be removed by this sweep"
        );
        assert!(
            !snapshots_dir
                .join(format!(".{session_id}.tmp-deadbeef"))
                .exists(),
            "an orphaned snapshot temp file must be removed"
        );
    }

    /// A missing `snapshots/` directory is the ordinary case for a state
    /// dir that has never captured a snapshot, and `sweep_snapshot_temp_files`
    /// is meant to treat it as a silent no-op rather than the generic warn
    /// branch it takes for any other read failure (see the function's own
    /// docs). This test pins only the panic/error-freedom half of that:
    /// startup calls this unconditionally on every boot, so it must never
    /// panic or block startup on a state dir this ordinary. It does not
    /// distinguish the silent-no-op path from the warn-and-continue path —
    /// a regression that routed `NotFound` into the warn branch would still
    /// pass here.
    #[tokio::test]
    async fn sweep_snapshot_temp_files_tolerates_a_missing_directory() {
        let tmp = tempfile::tempdir().unwrap();
        sweep_snapshot_temp_files(tmp.path()).await;
    }
}
