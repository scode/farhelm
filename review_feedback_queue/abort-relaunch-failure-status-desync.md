# A failed restore after a failed restart desyncs displayed status

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A session's status can change on its own after a supervisor restart.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0508-2b597e9-5788` (audit of Area 6, supervisor session state machine) as
`F8 / COR-ABORT-RELAUNCH-DESYNC`, tagged **definite**. Anchors and title: `service/core.rs:10254` — A failed restore
after a failed restart leaves displayed and stored status out of sync

This finding is low severity. When a restart begins, `begin_relaunch` durably opens a new "generation" (a per-restart
counter that fences off writes from older runs) and sets the row to `Launching`. If the relaunch then fails
"definitively", meaning nothing outside the supervisor changed, `relaunch` calls `abort_relaunch` to put the previous
run's outcome back in the row (for example "exited (stopped)").

Around core.rs:10245-10258, the in-memory outcome is set to the restored value only when `abort_relaunch` succeeds. If
it fails, the code logs a warning, "the session lists as unknown until it is restarted again", and republishes the entry
with whatever its outcome cell held before: typically the stopped/exited outcome that the restart's own stop wrote. So
clients are told the old status while the database says `Launching` at the new generation with no recorded pane. The log
line does not describe what is actually published. Nothing reconciles the two until the supervisor restarts. At that
point reload reads the `Launching` row, and the status the user sees changes on its own, to "unknown" or, through the
stale-pane path in F3/F4, to something worse.

Trigger: a database error on `abort_relaunch` right after a definitive restart failure. Either of two small changes
fixes it. One is to set the entry's outcome to `LastOutcome::Launching` in the error arm, mirroring the
ambiguous-failure branch, so memory matches what is durable. The other is to correct the log text. The first is
preferable. What the user sees: after a failed restart combined with a database error, the session's status can change
by itself after a supervisor restart.
