# A failed stop leaves a stale StopRequested intent

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A session whose agent later exits or crashes on its own is shown as "stopped by user" although the stop never took
effect.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0247-2b597e9-be5c` (audit of Area 1, process-tree kill sweep and teardown) as
`F5 / COR-STALE-INTENT`, tagged **possible**. Anchors and title: `service/sweep.rs:1732-1747`, `store.rs:531-534` — A
stop that fails leaves a stale `StopRequested`, so the agent's later natural exit is recorded as "stopped by user"

Farhelm records whether the user ended a run. The stop lifecycle, `stop_live_agent` (sweep.rs:1732-1747), first commits
a durable intent, `StopRequested`, to the session's database row. It then runs the kill sweep and finally commits
`StopCompleted`. The store's transition table (store.rs:531-534) interprets any exit it observes while the row says
`StopRequested` as the user's doing: an `ObservedExit` from `StopRequested` becomes "exited, stopped by user".

Nothing in the running supervisor withdraws the intent when the stop did not take effect. The only thing that clears it
is supervisor startup reconciliation, which maps a still-live pane under `StopRequested` back to `Running` through
`ConfirmRunning`. The ordinary listing path deliberately never sends `ConfirmRunning`.

There are two routes to a failed stop that leaves the agent alive. One is the aborted restart in F4. The other is
`stop_live_agent` returning `StopFailure::Sweep` when the root agent process was never signalled. For example, the
start-time re-read that must match before every signal (`signal_validated`) can fail with a read error. That error is
collected, the process is skipped, and the sweep reports failure. In either case the agent keeps running with
`StopRequested` recorded, and whenever it later ends by itself (it finishes, crashes, or the user quits it), the exit is
committed as a user stop. Exit outcomes are sticky, so the mislabel is permanent.

The annotation exists to tell the user whether they ended the run. A run the stop never reached ends up recorded as
user-stopped, which is the mislabel the intent/outcome design was built to prevent.

The premise is that the agent survives a failed stop, for example because the sweep erred before signalling its root.
The fix: on `StopFailure::Sweep`, and from a drop guard armed between `StopRequested` and `StopCompleted`, re-probe the
pane. If it is still alive, record `ConfirmRunning` (which maps `StopRequested` back to `Running`), mirroring what
reload does.

For the user, a session whose agent later exits or crashes on its own is shown as "stopped by user" although the stop
never took effect.
