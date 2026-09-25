# A second restart holds the host-wide directory lock while waiting

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Clicking Restart twice can freeze creates, deletes, other restarts and typing on that host until the first restart
finishes.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0508-2b597e9-5788` (audit of Area 6, supervisor session state machine) as
`F12 / COR-SECOND-RESTART-GLOBAL-LOCK`, tagged **definite**. Anchors and title: `service/core.rs:9780-9785`,
`service/core.rs:10077-10090` — A second restart of the same session holds the host-wide directory lock while waiting
for the first

The supervisor is the per-host process that owns agent sessions. It has two kinds of lock that matter here. The first is
`working_copy_operations`, also called "directory admission": one mutex for the whole supervisor, which every create,
restart and delete on the host takes. The second is the per-session "lifecycle claim" (`lifecycle_locks.claim(id)`),
which serializes Stop, Restart, Delete, rename and similar operations on one session. `restart_session` takes directory
admission first (core.rs:9780). The lock order is fixed as directory, then lifecycle, to avoid a cycle with restricted
creates. While still holding directory admission, it waits for the session's lifecycle claim (core.rs:9785). It drops
directory admission at core.rs:9840, before the stop and relaunch work.

That ordering works for a restart that gets the lifecycle claim immediately. It does not work for a second request that
queues behind one already in flight. A running restart moves its lifecycle claim into a supervisor-owned relaunch task
(core.rs:10077-10090) and holds it through the stop sweep, which includes the 5-second SIGTERM grace, SIGSTOP/SIGKILL
and confirmation polling, and then through the whole relaunch with its tmux round trips. A Stop holds the same claim
across its kill sweep. A second Restart of the same session waits on that claim while holding the host-wide mutex. The
second Restart can come from a double-click, a client retry after a timeout, or an agent restarting itself. For the
whole of the first operation, every create, every restart of any other session, and every delete on the host queues
behind it. Create currently runs inline on the helm connection's read loop, which is recorded separately as planned
work. So terminal input on that connection freezes too.

The fix is to never wait on the lifecycle claim while holding directory admission. One way: while holding admission, try
the claim without waiting. If it is busy, release admission, wait for the claim to free up, then retry the acquisition
in the documented order. The simpler way is to refuse a second concurrent restart of the same session with a `Conflict`
error. Users would see this as: clicking Restart twice can freeze creates, deletes, other restarts and typing on that
host until the first restart finishes.
