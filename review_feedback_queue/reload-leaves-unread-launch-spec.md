# Reload leaves unread launch specs after Interrupted/Exited

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After a reboot or an early shell exit, a session that never started its agent keeps a credential-bearing file on disk
until deleted.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0508-2b597e9-5788` (audit of Area 6, supervisor session state machine) as
`F21 / COR-RELOAD-SPEC-NOT-CLEANED`, tagged **definite**. Anchors and title: `service/core.rs:5373`,
`service/core.rs:5415-5446`, `service/core.rs:5491`, `service/core.rs:5613-5680`, `service/core.rs:5708-5724` — Startup
reconciliation cleans launch artifacts only after an Error outcome, so Interrupted/Exited launches keep their unread
spec

When the supervisor starts, `reload_sessions` reconciles every stored session row with what tmux shows. The only helper
that deletes a launch's spec and sentinel is `cleanup_launch_artifacts`, and reload calls it only for rows that end up
in the **error** state. That covers a sentinel found during the reboot conversion (`:5415-5446`), a row already in Error
(`:5491`), and a newly committed Error (`:5708-5724`). Every other final outcome leaves the current generation's spec in
place.

Two common shapes fall through. After a host reboot, `record_boot` converts every Launching, Running or StopRequested
row without a sentinel to **interrupted** (`:5373` and following). Nothing is deleted, so a launch whose shell had not
yet reached the shim when the host went down keeps its unread spec. On an unscoped launch (no systemd user manager:
every Mac, and Linux without `systemd --user`), a login shell that exited in its rc files leaves a dead pane. The
`wrapper_failure_detail` classifier, which would turn "dead pane plus unconsumed spec" into Error, only runs for scoped
launches. Reload therefore records an ordinary exit through `RediscoveredExit`/`ObservedExit` (`:5613-5680`), again
without cleanup. Once the pane is dead or gone, nothing can ever read that spec, and the startup sweep skips it because
the session still exists (see F20). The command line and session token stay on disk until the session is deleted.

The suggested fix: after reload commits any final outcome for a dead or missing pane, including the reboot conversion to
Interrupted, best-effort delete that generation's `.json` spec. The deletion must run after `wrapper_failure_detail` has
checked whether the spec exists, because that check is its evidence. The `.status` sentinel rules stay as they are. This
is the startup path only. F22 is the same gap in the runtime observers, which are separate code and need their own
change.
