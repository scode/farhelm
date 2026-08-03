//! Launch sentinel and launch-spec file helpers.
//!
//! A launch spec is written before an agent execs; a sentinel exists only
//! when the exec FAILED — the shim records the errno before exiting
//! (PLAN_M3.md item 10). Both are plain files under the state directory.
//! `crate::launch` owns their on-disk shape, the shim-side write, and the
//! raw sentinel read; what lives here is the service side of them — the
//! async read wrapper, outcome classification, and the removal and sweep
//! helpers. Nothing here touches tmux or the in-memory session map — see
//! `service::sweep` for the process-tree side of a launch's failure
//! classification.

use crate::store::LastOutcome;
use anyhow::Context;
use std::path::Path;
use tracing::{debug, warn};

/// The explanation for a launch that never reached the exec shim at all,
/// or `None` when this is not that shape (PLAN_M3.md item 10).
///
/// A gap the cgroup wrapper opened, and the reason it needs its own
/// classifier. Every other launch failure is reported by the SHIM, which
/// writes a sentinel before exiting (`crate::launch`'s module docs explain
/// why the shim and not the shell). `systemd-run` runs BEFORE the shim: a
/// wrapper that fails — the user manager died since the probe, the unit
/// name was refused, the scope could not be created — exits the pane with
/// no sentinel written and no `exec` ever attempted. Left alone, that
/// classifies as a plain `Exited` with whatever code `systemd-run` chose,
/// which is a lie about an agent that never ran, and leaves the launch
/// spec (the agent's full command line, credentials included) on disk with
/// nothing left to consume it.
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
/// Narrowed to SCOPED launches deliberately. Without a wrapper there is
/// nothing between the login shell and the shim but the shell itself, and an
/// unconsumed spec then means the user's rc files ended the shell — a
/// pre-existing M2 shape this build has no new evidence about and does not
/// reclassify.
///
/// A stat failure reports `None`: this is an inference of last resort, and
/// inventing an `error` classification from an unreadable state directory
/// would be exactly the guess the no-guessing rule forbids.
pub(crate) async fn wrapper_failure_detail(
    state_dir: &Path,
    id: &str,
    generation: i64,
    scoped: bool,
    pane_dead: bool,
) -> Option<String> {
    if !scoped || !pane_dead {
        return None;
    }
    let spec = crate::launch::spec_path_for_launch(state_dir, id, generation);
    match tokio::fs::try_exists(&spec).await {
        Ok(true) => Some(
            "the agent was never started: the launch never reached farhelm's exec shim, so \
             something before it — the transient cgroup scope wrapper, or the login shell \
             itself — exited first"
                .to_string(),
        ),
        Ok(false) => None,
        Err(e) => {
            debug!(
                session = %id, generation, error = %e,
                "could not tell whether this launch's spec was consumed; not classifying it \
                 as a wrapper failure"
            );
            None
        }
    }
}

/// Async wrapper around [`crate::launch::read_launch_sentinel`] for its
/// two service-side callers: `handlers`' `ListSessions` calls it on every
/// poll for every eligible session, which makes it the genuinely hot one;
/// `core`'s `reload_sessions` calls it only once per construction (or handoff), far
/// off any hot path, but shares this wrapper anyway so the two call sites
/// can never diverge on how the read reaches the filesystem.
///
/// `spawn_blocking` wraps what is usually a single `ENOENT`-returning
/// `read` (cheap in the overwhelmingly common case: no launch has ever
/// failed for this session) because a synchronous syscall run inline on
/// an async worker thread blocks every OTHER session's terminal
/// forwarding sharing that thread for however long the underlying I/O
/// takes — worth paying on `ListSessions`'s polling path even though any
/// one call is ordinarily fast.
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
/// `reload_sessions` and `handlers`' `ListSessions`) can never drift from what
/// the store would actually do with the reading once it is offered.
///
/// `false` only for an already-`Error` row (idempotent — nothing to gain)
/// and for a GENUINELY annotated `Exited` (a real stop: retained
/// knowledge, not an inference a sentinel could outrank). `true` for
/// everything else, INCLUDING `Interrupted` and an unannotated `Exited` —
/// PLAN_M3.md item 3 requires a late-discovered sentinel to still
/// supersede both, because neither is anything more than an inference
/// from an ordinary dead-or-vanished pane, exactly the evidence class a
/// sentinel is defined to beat.
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

/// Remove both files a launch's `Error` classification can leave behind:
/// the sentinel itself, and the per-launch SPEC file the shim's own
/// missing/malformed-spec early-return paths (or a failed unlink partway
/// through one) can leave stranded holding the agent's full command line,
/// credentials included. Called once a launch's `Error` outcome is
/// confirmed durably committed — nothing ever needs either file again
/// once the classification is settled — and also, idempotently, on every
/// row already found to be `Error` on load: a crash between an EARLIER
/// pass's commit and the cleanup that should have followed it can leave
/// one or both files behind for an arbitrary number of startups, and this
/// is what finally sweeps them. Best-effort throughout
/// (`best_effort_remove`): a failure here is logged, never fatal, and
/// never blocks a reply — both files are cosmetic once the DURABLE
/// outcome already says what happened.
pub(crate) async fn cleanup_launch_artifacts(state_dir: &Path, id: &str, generation: i64) {
    let spec_path = crate::launch::spec_path_for_launch(state_dir, id, generation);
    let status_path = crate::launch::status_path_for_spec(&spec_path);
    best_effort_remove(&status_path, "consumed launch sentinel").await;
    best_effort_remove(&spec_path, "leftover launch spec").await;
}

/// Clear the launch artifacts sitting at ONE launch's spec and sentinel
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
/// classification, so if only one of the two removals gets to run, that is
/// the one worth having run.
pub(crate) async fn clear_launch_artifacts_fail_closed(
    state_dir: &Path,
    id: &str,
    generation: i64,
) -> Result<(), String> {
    let spec_path = crate::launch::spec_path_for_launch(state_dir, id, generation);
    let status_path = crate::launch::status_path_for_spec(&spec_path);
    remove_fail_closed(&status_path, "the previous launch's sentinel").await?;
    remove_fail_closed(&spec_path, "the previous launch's spec").await
}

/// Remove every launch spec and sentinel belonging to `session_id`, across
/// all of its generations, fail-closed.
///
/// Delete's cleanup, and the one place that has to enumerate rather than
/// derive: a session that was restarted owns one file pair per launch
/// (`launch::spec_path_for_launch`), and the row that would say how many is
/// about to be removed. Every failure — including a directory that cannot
/// be read at all — is returned rather than logged, because these files
/// hold agent command lines users put credentials into and delete is the
/// last moment anything will ever come back for them.
pub(crate) async fn remove_launch_artifacts_for_session(
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
        if crate::launch::parse_launch_file_name(&name).is_some_and(|(id, _)| id == session_id) {
            remove_fail_closed(&entry.path(), "launch file").await?;
        }
    }
}

/// Best-effort credential-hygiene cleanup: remove `path`, treating its
/// absence as success (the shim may already have consumed and unlinked
/// it) and logging anything else as a warning naming both the file and
/// what it was, rather than propagating — every call site here is itself
/// already unwinding a different failure, and this cleanup must not mask
/// that original error with an unrelated filesystem one.
pub(crate) async fn best_effort_remove(path: &Path, what: &str) {
    match tokio::fs::remove_file(path).await {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            warn!(path = %path.display(), error = %e, "could not remove {what}");
        }
    }
}

/// Remove `path`, tolerating its absence (the shim usually already
/// unlinked it — see launch.rs) but treating any OTHER failure as fatal,
/// unlike `best_effort_remove`'s log-and-continue.
///
/// Used only by `DeleteSession`: a leftover launch spec may hold the
/// agent's full command line, credentials included, and delete is the
/// last moment anything will ever come back to clean it up — a caller
/// here cannot shrug off a removal failure the way create's failure-
/// unwind path does (which returns a different, already-fatal error
/// either way).
pub(crate) async fn remove_fail_closed(path: &Path, what: &str) -> Result<(), String> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("removing {what} ({}): {e}", path.display())),
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
/// Sentinels (`.status` files) are NEVER touched here, regardless of
/// ownership — PLAN_M3.md item 5's durability promise for them would be
/// worthless if a blanket startup sweep could erase the very evidence a
/// later classifier needs to read; their lifecycle (supersede on
/// relaunch, or explicit delete) belongs entirely to that future
/// consumer, never to this best-effort hygiene pass.
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
/// Best-effort and log-only: this sweep is credential hygiene (specs hold
/// full agent command lines), so a failure that leaves debris behind must
/// at least say so in the log, but never fails startup over it.
pub(crate) async fn sweep_launch_dir(
    launch_dir: &Path,
    sessions: &std::collections::HashSet<String>,
) {
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
            true
        } else if let Some((id, _generation)) = crate::launch::parse_launch_file_name(&name) {
            // Names are `<id>.<generation>.json|status` now that launch
            // files are per-LAUNCH rather than per-session
            // (`launch::spec_path_for_launch`), and the sweep's question is
            // still the same one: does the SESSION still exist? A file
            // belonging to a live session's older generation is not swept
            // here — the restart that superseded it removes its own
            // predecessor, and sweeping by generation would mean deciding,
            // from a directory listing, which launch is current.
            //
            // Only specs are ever removed; `.status` sentinels are never
            // this sweep's to remove (see the function's own docs), which
            // the extension check preserves.
            !sessions.contains(id) && name.ends_with(".json")
        } else {
            false
        };

        if should_remove && let Err(e) = tokio::fs::remove_file(entry.path()).await {
            warn!(path = %entry.path().display(), error = %e,
                "could not remove orphaned launch-dir entry");
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
    /// delete, may ever remove one. A staged temp file and an ORPHANED
    /// spec (its session id absent from `sessions`) are seeded alongside
    /// it and must both go, proving the sweep does not simply skip the
    /// whole directory.
    #[tokio::test]
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
        // A LATER generation of a session that still exists: this sweep
        // asks about session ownership, never about which launch is
        // current, so a live session's files survive whatever their
        // generation (the restart that supersedes them cleans up its own
        // predecessor).
        std::fs::write(launch_dir.join("live.3.json"), b"{}").unwrap();
        std::fs::write(launch_dir.join(".orphan.0.json.tmp-deadbeef"), b"partial").unwrap();
        // Unrecognized names are never this sweep's to remove.
        std::fs::write(launch_dir.join("not-ours"), b"?").unwrap();

        let live: std::collections::HashSet<String> = ["live".to_string()].into_iter().collect();
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
            "a staged temp file must always be removed"
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
    #[test]
    fn launch_file_names_round_trip_through_the_parser() {
        let state = std::path::Path::new("/state");
        let spec = crate::launch::spec_path_for_launch(state, "sess-1", 7);
        let status = crate::launch::status_path_for_spec(&spec);
        for path in [&spec, &status] {
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

    /// Item 22's restart race: a spec whose session id IS still present
    /// in `sessions` must survive the sweep untouched — a supervisor
    /// restart does not kill tmux, so the login shell behind that session
    /// can still be mid-flight toward reading this exact spec, arbitrarily
    /// long after the window itself was created.
    #[tokio::test]
    async fn sweep_launch_dir_preserves_a_spec_for_a_surviving_session() {
        let tmp = tempfile::tempdir().unwrap();
        let launch_dir = tmp.path().join("launch");
        std::fs::create_dir(&launch_dir).unwrap();
        std::fs::write(launch_dir.join("live.json"), b"{}").unwrap();

        let mut sessions = std::collections::HashSet::new();
        sessions.insert("live".to_string());
        sweep_launch_dir(&launch_dir, &sessions).await;

        assert!(
            launch_dir.join("live.json").exists(),
            "a spec for a session still on record must survive — its shim may still be \
             mid-flight toward reading it"
        );
    }

    /// The wrapper-failure predicate, pinned in all three of its arms.
    ///
    /// Runs everywhere, systemd or not, because the shape it recognizes is
    /// entirely on-disk — which is also why it needs its own test: the e2e
    /// version can only run where a user manager exists, and this is the
    /// classifier that decides whether an agent that never started is
    /// reported as `error` or as a plain exit.
    ///
    /// The unscoped arm is the one that is easy to get wrong in the
    /// permissive direction. Without a wrapper there is nothing between the
    /// login shell and the shim but the shell, so an unconsumed spec there
    /// means the user's rc files killed the shell — a pre-existing M2 shape
    /// this build has no new evidence about and must not reclassify.
    #[tokio::test]
    async fn a_wrapper_failure_is_recognized_only_by_its_full_shape() {
        let state = StateDir::new();
        let id = uuid::Uuid::new_v4().to_string();
        let spec = crate::launch::spec_path_for_launch(state.path(), &id, 0);
        std::fs::create_dir_all(spec.parent().expect("launch dir")).expect("launch dir");

        assert_eq!(
            wrapper_failure_detail(state.path(), &id, 0, true, true).await,
            None,
            "no spec on disk means the shim consumed it and really did run"
        );

        std::fs::write(&spec, b"{}").expect("plant an unconsumed spec");
        assert_eq!(
            wrapper_failure_detail(state.path(), &id, 0, false, true).await,
            None,
            "an unscoped launch has no wrapper to have failed"
        );
        assert_eq!(
            wrapper_failure_detail(state.path(), &id, 0, true, false).await,
            None,
            "an ABSENT pane means the window or the whole tmux server was destroyed, which \
             strands a spec from a launch that was interrupted rather than failed"
        );
        assert!(
            wrapper_failure_detail(state.path(), &id, 0, true, true)
                .await
                .is_some_and(|detail| detail.contains("never reached farhelm's exec shim")),
            "a scoped launch whose spec was never consumed, under a dead pane, never started"
        );

        // Per LAUNCH, like every other artifact keyed on the generation: a
        // spec left by generation 0 must not paint generation 1 as failed.
        assert_eq!(
            wrapper_failure_detail(state.path(), &id, 1, true, true).await,
            None,
            "a previous generation's leftover spec is not this launch's evidence"
        );
    }
}
