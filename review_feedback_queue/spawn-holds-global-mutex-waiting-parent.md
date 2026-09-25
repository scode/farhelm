# A spawn waiting on its busy parent holds the host-wide mutex

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

While one session is being stopped, creating, deleting or restarting any other session on the same host can stall until
that stop finishes.

## Details

Paths are relative to `crates/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0349-2b597e9-145b` (audit of Area 3, agent upcall relay and
session-credential authority) as `F8 / COR-ADMIT-GLOBAL-LOCK`, tagged **possible**. Anchors and title:
`farhelm-supervisor/src/service/core.rs:6621-6647`, `farhelm-supervisor/src/service/handlers.rs:1074`,
`farhelm-supervisor/src/service/core.rs:9780-9840` — A spawn waiting on its busy parent holds the host-wide
create/delete mutex

Two locks matter here. `working_copy_operations` is a single supervisor-wide mutex that every create, delete, and
restart on the host passes through (it serializes working-copy bookkeeping). The **lifecycle claim** (`lifecycle_locks`)
is a per-session lock held by whatever operation is currently changing that session. The documented order is mutex
first, then lifecycle claim.

`admit_create` handles the create half of `farhelm spawn` (`core.rs:6621-6647`). It acquires the host-wide mutex, then,
**still holding it**, waits for the asking session's lifecycle claim. It needs that claim so it can re-check the
credential under it. A stop holds the lifecycle claim for its whole process-tree kill sweep (`handlers.rs:1074`), and
that can take seconds, or longer if tmux is wedged. So if an agent runs `farhelm spawn` while its own session is being
stopped, a likely moment, since the stop kills the agent's process tree while it may be mid-spawn, the spawn parks while
holding the host-wide mutex. Every other create, delete, and restart on the host then waits behind that one stop. The
mutex's docs say it is held only for bookkeeping and "never across a tmux round trip". Restart (`core.rs:9780-9840`)
releases it before its slow tmux work.

The impact is that while one session is being stopped, creating, deleting, or restarting any other session on the same
host can stall until the stop finishes. The open premise is how long real stops take; on a healthy host it is seconds.
The suggested fix keeps the lock order and avoids waiting under the mutex:

1. Take the mutex and try the parent claim without waiting.
2. If the claim is busy, release the mutex, wait for the claim, and retry.

Restater note: the pattern is not unique to spawn, and the mutex's "never across a tmux round trip" doc is already
stale. Restart (`core.rs:9780-9790`) also takes the mutex and then waits for the target session's lifecycle claim while
holding it, so a restart of a session that is being stopped parks the mutex the same way. Delete
(`handlers.rs:1413-1426`) deliberately holds the mutex across its entire teardown sweep and says so in a comment. A
host-wide stall lasting one sweep is therefore already accepted for delete. What this finding adds is that a plain
**stop**, which never takes the mutex, can now hold it indirectly through a parked spawn. A fix may need to cover
restart's wait too, or the maintainer may judge the existing ordering acceptable.
