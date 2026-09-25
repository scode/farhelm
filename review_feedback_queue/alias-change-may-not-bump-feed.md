# An alias change can fail to reach other open clients

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After renaming a host, other open windows can keep showing the old name for a while.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0535-2b597e9-a2a7` (audit of Area 7, helm state) as
`F17 / COR-ALIAS-CHANGE-NO-BUMP`, tagged **possible**. Anchors and title: `manager.rs:1552`, `hosts.rs:640` — An alias
change can fail to reach other open clients

`sync_registry` copies an alias-only edit into the actor's `handle.row` (the `else { handle.row = row; }` branch at
`manager.rs:1552`) without marking the fleet shape as changed, so it does not bump the fleet revision. `set_alias`
compensates itself. It reads the published alias before and after its own `sync_registry` and bumps if they differ
(`hosts.rs:640` onward). But it reads "before" _after_ `update_alias` has committed, and it holds only this host's lock.

Suppose any other reconcile runs in that gap, triggered by any host edit elsewhere, even a no-op. That reconcile reads
the new alias from the database, puts it into `handle.row`, and bumps nothing. `set_alias` then sees `before == after`
and also bumps nothing. No client is told the name changed.

Suggested fix: have `sync_registry` set `shape_changed` whenever the alias (or any other displayed registry field)
changes, and drop the before/after comparison from `set_alias`.

User-visible consequence: after renaming a host, other open windows can keep showing the old name until something else
wakes them.
