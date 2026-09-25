# The sweep silently drops an unreadable root process

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

In rare cases a stop, delete or tab close reports success while the terminal's own process tree is still running.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0247-2b597e9-be5c` (audit of Area 1, process-tree kill sweep and teardown) as
`F25 / SEC-ROOT-READ-ERR`, tagged **possible**. Anchors and title: `service/sweep.rs:1278-1291` — If the pane process's
identity read fails, the sweep silently drops its root and can still report success

Before killing anything, `reap_process_tree` reads the start time of the pane's process (the pid tmux reported as
`pane_pid`) so it can use that process as the root of its parent-pid walk (sweep.rs:1278-1291). If `procs::read_process`
returns `Err`, the arm at line 1284 logs at debug level and continues with no root. That means a real failure, such as a
permission error or a malformed `/proc` row, not "process gone", which comes back as `Ok(None)`. The walk then has
nothing to start from, so processes under the pane that carry no marker are never visited. The sweep can still return
`Ok(())`.

Everywhere else in the module, an unreadable process is an error and never "gone":

- Walk expansion pushes read failures into `soft_errors`.
- `signal_validated` returns `Err`.
- `confirm_gone` keeps the pid and reports it.

The module's docs state the principle: "an unreadable process it meant to trust cannot disappear into a falsely clean
result". This arm contradicts it. Stop, Delete and Close Tab would report "nothing left running" after skipping the one
process they were asked to reap, with only a debug log.

The open premise is how often a read of a user-owned pid directory fails with something other than ENOENT/ESRCH. Treat
`Err` from the root read as a sweep failure, by carrying it into the aggregated errors or passing it to
`kill_process_tree` as a soft error, so the operation fails and can be retried. Keep `Ok(None)` (gone) silent.

For the user, in rare cases a stop, delete or tab close reports success while the terminal's own process tree is still
running.
