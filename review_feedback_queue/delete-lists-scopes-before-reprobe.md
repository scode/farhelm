# Delete lists systemd scopes before its one-time re-probe

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Delete reports the session removed while a background process from an earlier run or a closed tab keeps running, with
nothing left to retry from.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0247-2b597e9-be5c` (audit of Area 1, process-tree kill sweep and teardown) as
`F1 / COR-DELETE-REPROBE`, tagged **definite**. Anchors and title: `service/teardown.rs:273-293`,
`service/sweep.rs:1308-1310` — Delete lists the session's systemd scopes before the one-time re-probe, so a successful
re-probe kills an incomplete list

On hosts with a systemd user manager, Farhelm runs each agent launch and each terminal tab inside its own transient
systemd "scope" unit. A scope is a cgroup that systemd can signal as a whole: `farhelm-<session>-<generation>.scope` for
each launch generation (every restart is a new generation), `farhelm-<session>-tab-<tab>.scope` for each tab. Deleting a
session normally kills its processes with a portable "sweep" that walks down the process tree from the tmux pane and
also scans every same-user process's environment for the `FARHELM_SESSION_ID` marker variable Farhelm set at launch. A
process that both left the tree (double-forked and reparented to init) and wiped its environment is invisible to that
sweep. Killing the scope is the only way Delete can reach it.

Whether this host has a usable user manager is decided once per supervisor process by `ScopeManager` (scope.rs), which
caches the answer. A cached "no" can be overturned exactly once, through `reprobe()`, and only by a caller that holds
durable evidence a scope existed: the session's database row recording its launch as scoped.

Delete (`teardown_session`) gathers scope names first and kills them later, and the order is wrong for the re-probe. It
first asks the manager for every unit matching the session's tab glob and launch-generation glob. Teardown's own
comments call these globs the load-bearing source, because they find tab scopes whose tmux window has already gone and
the scopes of older generations. When the cached verdict is "no manager", `units_matching` fails at once without asking
systemd, and the `Err(e) if !available()` arm treats that as "this host has no manager", logs at debug level, and
continues with neither glob's results. Only afterwards does `reap_process_tree` (sweep.rs:1308-1310) see the
row-recorded unit and call `reprobe()`. If the re-probe succeeds, it kills the recorded unit plus the other names it was
given, but those now hold only the tab names tmux still listed. The globs are never re-run. The sweep completes, and
Delete removes the row.

This is exactly the stale-negative situation the re-probe exists for, such as a transient probe failure when the
supervisor started. A daemon with a scrubbed environment inside an older generation's scope or a closed tab's scope
keeps running, and once the row is gone nothing can find it again. Teardown's own comment says that publishing "deleted"
over a cgroup nobody enumerated is exactly what must not happen.

Settle the verdict before enumerating. In `teardown_session`, when `available()` is false and `entry.scope` is `Some`,
call `reprobe()` first and then run the globs. Alternatively, have `reap_process_tree` re-run the enumeration after a
positive re-probe. A glob failure after a successful re-probe should refuse the Delete, as the existing path already
does when a manager is known to be available.

For the user, Delete reports the session removed while a background process from an earlier run or a closed tab keeps
running, and there is nothing left to retry from.
