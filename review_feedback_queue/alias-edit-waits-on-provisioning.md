# Renaming a host alias blocks behind its provisioning run

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Renaming a host while it is being updated appears to hang until the update finishes.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0535-2b597e9-a2a7` (audit of Area 7, helm state) as
`F4 / COR-ALIAS-WAITS-PROVISIONING`, tagged **possible**. Anchors and title: `hosts.rs:636`, `manager.rs:2143` —
Renaming a host's alias blocks behind a provisioning run on that host

Setting a host's display alias (`POST /api/hosts/{id}/alias`, `set_alias` at `hosts.rs:630`) takes
`host_write_lock(host)` at `hosts.rs:636`. That is the same lock a provisioning run holds for its entire duration (F2),
so an alias change on a host being updated waits until the update finishes, with no feedback.

The registry side of the alias write does not need this lock:

- The alias is display-only. `reconfigured()` (`manager.rs:1311`), which decides whether an edit forces a reconnect,
  compares destination, remote binary and remote state dir, but not the alias.
- `update_alias` checks alias uniqueness inside its own database transaction.
- `sync_registry` already serialises against itself through the reconcile mutex.

Only retarget and remove are documented as needing the lock.

Suggested fix: drop `host_write_lock` from `set_alias`. This is independent of F2: even with a separate registry lock,
the alias write would not need to take it.

User-visible consequence: renaming a host while it is being updated appears to hang until the update finishes.

Restater note: the lock does currently do one thing for `set_alias`. It serialises two alias edits _on the same host_,
and `set_alias`'s before/after snapshot comparison (which decides whether to wake other clients, see F17) relies on
that. Dropping the lock without also fixing F17 would widen F17's race to concurrent same-host alias edits. Fix F17
first, or together with this.
