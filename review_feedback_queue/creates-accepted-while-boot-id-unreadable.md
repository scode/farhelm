# Creates accepted while the boot id is unreadable are later marked interrupted

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Sessions created in that window later show as interrupted while their agents are still running.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0508-2b597e9-5788` (audit of Area 6, supervisor session state machine) as
`F6 / COR-BOOTID-UNREADABLE-CREATE`, tagged **possible**. Anchors and title: `service/core.rs:5333-5346`,
`service/core.rs:9841`, `service/core.rs:8389`, `store.rs:5230` — While the boot id is unreadable, creates are accepted
and later misclassified as interrupted

The supervisor is the per-host process that owns agent sessions. It keeps them in a SQLite database. It decides whether
the machine rebooted by comparing the host's "boot id" (a random identifier the Linux kernel assigns to each boot) with
the id stored when it last started. If the two differ, every session still marked launching, running or stop-requested
is assumed dead. `SessionStore::record_boot` then converts all of them to `Interrupted` in one blanket `UPDATE`.
`Interrupted` is a terminal state: the transition table accepts no later exit code or stop annotation for such a row.

If reading the current boot id fails at startup, `reload_sessions` (core.rs:5333-5346) switches to a degraded, read-only
mode. It does not claim a reboot, does not store a new boot id, and clears `may_record()`, the flag that says "this
process may write what it observes". Restart honours that flag and refuses with "this supervisor is not recording
session state … it will not relaunch a session it cannot durably account for" (core.rs:9841). Create has no such check.
`launch_reserved` (core.rs:8389) still inserts a `Launching` row and commits it as `Running`, so new sessions become
durable rows while the database still holds the previous boot's id.

Here is how that goes wrong. Suppose the machine reboots and the supervisor's first start after the reboot cannot read
the boot id. Sessions created during that run are stored as `Running`. The next time the supervisor starts in the same
boot and the read works, the stored id (from before the reboot) differs from the current one. The supervisor concludes a
reboot happened, and `record_boot` (store.rs:5230) marks these new sessions `Interrupted` even though their agents are
still running. Reload then finds the live pane but cannot move the row back to running. When the agent later exits, the
exit is not recorded either.

This depends on a boot-id read failing at startup, which the code handles on purpose but which should be rare. The
simplest fix is for the create path (`create_session_admitted`, plus the allocation branch of
`reconcile_github_checkout`) to refuse with the same `may_record()` check that restart uses. Another option is to store
the boot id on each created row, so that `record_boot` converts only rows launched under an older boot. What the user
sees: sessions created in that window later show as "interrupted" while their agents are still running.
