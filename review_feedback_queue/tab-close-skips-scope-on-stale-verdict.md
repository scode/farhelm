# Tab close skips the tab's cgroup scope on a stale negative verdict

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Closing a tab can leave its background processes running on a host whose manager probe once failed, with no error shown.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0247-2b597e9-be5c` (audit of Area 1, process-tree kill sweep and teardown) as
`F2 / COR-TAB-SCOPE`, tagged **definite**. Anchors and title: `service/core.rs:12043-12047`,
`service/core.rs:11991-11998`, `service/sweep.rs:1308-1327` — Tab close never kills the tab's cgroup scope while the
cached manager verdict is negative, contrary to `reap_tab_tree`'s documented contract

Each terminal tab (an extra shell window next to the agent) gets its own systemd scope, a cgroup systemd can kill as a
unit. That is the only thing that catches a tab descendant that daemonized and wiped its environment, because the
portable sweep finds processes only by walking the process tree or by reading Farhelm's environment markers.
`reap_tab_tree` is the shared kill routine for three callers: the user's Close Tab, the ticker's automatic reap of tabs
whose shell exited (the ticker is the supervisor's periodic background loop), and the unwind of a failed tab open.

The docs on `reap_tab_tree` (core.rs:11991-11998) promise that the tab's scope name is derived and tried "even when this
supervisor's own availability probe says there is no user manager". The reasons given are that the scope may predate
this supervisor, or the probe may have hit a briefly unreachable manager, and "skipping one that does costs the
containment guarantee". The code does not do this. It passes the unit as `ScopeUnits::derived(...)`
(core.rs:12043-12047), and `reap_process_tree` (sweep.rs:1308-1327) silently drops derived names whenever the cached
verdict says "no manager" and there is no row-recorded unit. A tab never has a recorded unit, so it never gets the
one-time re-probe that recorded launch units get. Even if the name got through, `ScopeManager::exists` and `kill` refuse
to run on a negative verdict.

A concrete case: supervisor A opens a tab while the user manager works. The supervisor restarts, and the new supervisor
B's first probe fails transiently. From then on, closing that tab, or typing `exit` in it (which triggers the ticker's
reap), skips its cgroup. A reparented process with a scrubbed environment survives. So does a non-dumpable one such as
`ssh-agent` or `gpg-agent`, whose environment its own user cannot read. The close still reports success.

Either give tab units the same one-time re-probe right as a recorded launch unit (a third `ScopeUnits` provenance, or
pass the tab unit as recorded), or correct the `reap_tab_tree` docs and the close's promise to state the residual.

For the user, closing a tab can leave its background processes running, with no error, on a host whose manager probe
once failed.
