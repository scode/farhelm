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

use crate::store::{LastOutcome, SessionStore};
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
/// A stat failure is inconclusive, not a proven absence. Callers may preserve
/// their last-resort exit inference, but must not settle future reads from it:
/// an unreadable state directory cannot prove that the shim consumed its spec.
pub(crate) async fn wrapper_failure_detail(
    state_dir: &Path,
    id: &str,
    generation: i64,
    scoped: bool,
    pane_dead: bool,
) -> anyhow::Result<Option<String>> {
    if !scoped || !pane_dead {
        return Ok(None);
    }
    let spec = crate::launch::spec_path_for_launch(state_dir, id, generation);
    match tokio::fs::try_exists(&spec).await {
        Ok(true) => Ok(Some(
            "the agent was never started: the launch never reached farhelm's exec shim, so \
             something before it — the transient cgroup scope wrapper, or the login shell \
             itself — exited first"
                .to_string(),
        )),
        Ok(false) => Ok(None),
        Err(e) => {
            debug!(
                session = %id, generation, error = %e,
                "could not tell whether this launch's spec was consumed; not classifying it \
                 as a wrapper failure"
            );
            Err(e.into())
        }
    }
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
/// creates before unlinking an actual sentinel; retain both files if its read
/// or settlement fails, or its durable generation no longer matches.
///
/// Startup and periodic observers share this gate, including already-Error
/// paths. Returns true only after evidence preservation and both removals
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
    // the same outcome. Keep both files if preservation cannot commit.
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
    let spec_path = crate::launch::spec_path_for_launch(state_dir, id, generation);
    let status_path = crate::launch::status_path_for_spec(&spec_path);
    // Try both even when one fails: either can contain launch credentials.
    let status_removed = best_effort_remove(&status_path, "consumed launch sentinel").await;
    let spec_removed = best_effort_remove(&spec_path, "leftover launch spec").await;
    status_removed && spec_removed
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
        if crate::launch::parse_launch_file_name(&name).is_some_and(|(id, _)| id == session_id)
            || staged_name_belongs_to(&name, session_id)
        {
            remove_fail_closed(&entry.path(), "launch file").await?;
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

/// Best-effort credential-hygiene cleanup: remove `path`, treating its
/// absence as success (the shim may already have consumed and unlinked
/// it) and logging anything else as a warning naming both the file and
/// what it was, rather than propagating — every call site here is itself
/// already unwinding a different failure, and this cleanup must not mask
/// that original error with an unrelated filesystem one. Returns whether
/// removal succeeded so a background observer can stop retrying only then.
pub(crate) async fn best_effort_remove(path: &Path, what: &str) -> bool {
    match tokio::fs::remove_file(path).await {
        Ok(()) => true,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => true,
        Err(e) => {
            warn!(path = %path.display(), error = %e, "could not remove {what}");
            false
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
            // Names are `<id>.<generation>.json|status`: launch files are
            // per-LAUNCH (`launch::spec_path_for_launch`). Two rules:
            //
            // - A session no longer on record loses its SPECS only. Its
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
                None => name.ends_with(".json"),
                Some(current) => generation < *current,
            }
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
        // A LATER generation of a session that still exists: this sweep
        // asks about session ownership, never about which launch is
        // current, so a live session's files survive whatever their
        // generation (the restart that supersedes them cleans up its own
        // predecessor).
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
        let unrelated = launch_dir.join(".not-a-launch-file.tmp-deadbeef");
        for path in [&own_spec, &own_staged, &other_staged, &unrelated] {
            std::fs::write(path, b"credential-bearing launch data").unwrap();
        }

        remove_launch_artifacts_for_session(tmp.path(), session)
            .await
            .unwrap();

        assert!(!own_spec.exists());
        assert!(!own_staged.exists());
        assert!(other_staged.exists());
        assert!(unrelated.exists());
    }

    /// Spec: for a session still on record, the sweep keeps its CURRENT
    /// generation's spec and sentinel and removes both for earlier
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
            "live.2.json",
            "live.2.status",
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
    #[farhelm_testtrace::test]
    async fn a_wrapper_failure_is_recognized_only_by_its_full_shape() {
        let state = StateDir::new();
        let id = uuid::Uuid::new_v4().to_string();
        let spec = crate::launch::spec_path_for_launch(state.path(), &id, 0);
        std::fs::create_dir_all(spec.parent().expect("launch dir")).expect("launch dir");

        assert_eq!(
            wrapper_failure_detail(state.path(), &id, 0, true, true)
                .await
                .unwrap(),
            None,
            "no spec on disk means the shim consumed it and really did run"
        );

        std::fs::write(&spec, b"{}").expect("plant an unconsumed spec");
        assert_eq!(
            wrapper_failure_detail(state.path(), &id, 0, false, true)
                .await
                .unwrap(),
            None,
            "an unscoped launch has no wrapper to have failed"
        );
        assert_eq!(
            wrapper_failure_detail(state.path(), &id, 0, true, false)
                .await
                .unwrap(),
            None,
            "an ABSENT pane means the window or the whole tmux server was destroyed, which \
             strands a spec from a launch that was interrupted rather than failed"
        );
        assert!(
            wrapper_failure_detail(state.path(), &id, 0, true, true)
                .await
                .unwrap()
                .is_some_and(|detail| detail.contains("never reached farhelm's exec shim")),
            "a scoped launch whose spec was never consumed, under a dead pane, never started"
        );

        // Per LAUNCH, like every other artifact keyed on the generation: a
        // spec left by generation 0 must not paint generation 1 as failed.
        assert_eq!(
            wrapper_failure_detail(state.path(), &id, 1, true, true)
                .await
                .unwrap(),
            None,
            "a previous generation's leftover spec is not this launch's evidence"
        );
    }
}
