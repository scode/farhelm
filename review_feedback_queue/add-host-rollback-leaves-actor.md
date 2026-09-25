# add_host's rollback leaves a concurrently started actor running

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After a failed "add host", the host the helm said it did not register can still appear in the list, possibly connected,
and cannot be removed until the next host change.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0535-2b597e9-a2a7` (audit of Area 7, helm state) as
`F13 / COR-ADD-ROLLBACK-LEAVES-ACTOR`, tagged **possible**. Anchors and title: `hosts.rs:498-499` — add_host's rollback
deletes the row but not an actor a concurrent reconcile already started

`add_host` commits the new registry row, then calls `sync_registry`. If that reconcile fails, it rolls back by deleting
the row with `remove_ssh_host` (`hosts.rs:498-499`) and tells the caller the host was not registered. It does not stop
any actor. Another handler's `sync_registry` could have run between the commit and this failure. That could be another
add, an alias edit, a retarget, or provisioning registering a host. In that case an actor for the new row already
exists, and the rollback leaves it behind:

- If it had already connected, it keeps serving (F14).
- If not, its next row reload finds the row gone and it retires. It still stays in the actor map as a "retired" entry,
  visible in `/api/hosts`.
- Either way, Remove answers 404 (the row is gone), and so does Retry (`revive` finds no row).

It lingers until the next successful reconcile, which drops actors whose rows are gone. Provisioning's own rollback gets
this right: it calls `stop_actor` (`provisioning/service.rs:548`). The trigger needs `sync_registry` to fail, and its
only fallible step is the `list_hosts` database read, so this is rare.

Suggested fix: in the rollback, after `remove_ssh_host` succeeds, call `state.manager.stop_actor(host).await`.

User-visible consequence: after a failed "add host", the host the helm said it did not register can still appear in the
list, possibly connected, and cannot be removed until the next host change.
