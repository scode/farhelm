# Every Delete holds the host-wide directory lock through teardown

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

While any session is being deleted, starting or restarting any other session on that host appears to hang for several
seconds or more.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0443-2b597e9-b20b` (audit of Area 5, Delete, owned-checkout archival and
attachment removal) as `F12 / COR-DELETE-GLOBAL-LOCK`, tagged **possible**. Anchors and title:
`service/handlers.rs:1424-1428`, `service/teardown.rs:89-102`, `service/core.rs:6621`, `service/core.rs:9780`,
`service/core.rs:4336-4343` — Every Delete holds the host-wide directory lock through its whole teardown

The supervisor has one global async mutex, `working_copy_operations`, called directory admission. It serializes checkout
allocation, membership changes and the decision to archive a checkout when its last reference goes.

The Delete handler (handlers.rs 1424-1428) takes this lock for every delete, including sessions that have no checkout.
While holding it, the handler waits for the session's lifecycle claim, the per-session lock shared with stop and
restart. It then keeps the lock through the whole teardown:

- cancelling uploads
- the tmux calls
- the process-tree sweep, which can wait up to its 5-second kill grace for processes that ignore termination
- archival
- the final transaction

Every create (`admit_create`, core.rs 6621) and every restart (core.rs 9780) take the same lock. As a result:

- All creates and restarts on the host queue behind any delete in progress.
- Deletes run one at a time.
- A delete that is waiting for a session busy with a stop or restart stalls the whole host in the meantime.

The documentation understates this. The teardown doc (teardown.rs 89-102) says only that "concurrent fresh creates
wait". The mutex's own doc (core.rs 4336-4343) says it is never held "across a tmux round trip", and that delete/restart
acquisition "arrives with the teardown slice", which is out of date.

The effect is that bulk deletes, or Replace (create then delete), can stall new sessions and restarts for several
seconds or more. The stale docs also mislead whoever changes this code next.

Open premise: the maintainer may accept this breadth, since the lock is held on purpose. The code documents a narrower
cost than it actually has.

Suggested change, in order of preference:

- Run the sweep and the tmux kill outside directory admission. Then take directory admission plus the session lock,
  recheck that the session still exists, archive, and commit.
- Take the directory lock only when the session has checkout memberships, rechecked once the lock is held.
- At minimum, correct both doc comments.
