# Respawning a dead actor drops its client without retiring it

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After an actor crash followed by a retry or reconcile, a stale connection can linger and answer agent requests on that
host's behalf.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0535-2b597e9-a2a7` (audit of Area 7, helm state) as
`F15 / COR-RESPAWN-SKIPS-RETIRE`, tagged **possible**. Anchors and title: `manager.rs:1488`, `manager.rs:2591` —
Respawning or reviving a dead actor aborts its supervisor task without retiring a still-published client

Each actor runs under a small **supervisor task** (`spawn_actor`, `manager.rs:1669-1716`). When the actor ends,
including by panicking, the supervisor task publishes `Retired`, takes the published `client` (the live connection) out
of the status, and calls `retire_withdrawn` on it. That call is needed because, per its docs (`manager.rs:1271-1281`),
dropping the last `Arc` does not reliably close the connection: an agent answer in progress can keep the transport open.

`sync_registry`'s respawn (`manager.rs:1488`) and `revive` (`manager.rs:2591`) both decide an actor is "dead" when its
nudge channel is closed or its task is finished. There is a gap in which the first is already true: after the actor has
panicked and dropped its nudge receiver, but before the supervisor task has run its retirement code. If a respawn or
revive lands in that gap, `previous.task.abort()` cancels the supervisor task before it retires the client. The old
handle, and the client in its status, are then simply dropped. Unlike `stop_actor`, neither path takes and retires the
client itself.

The window is short (between the actor's panic and the supervisor task being scheduled), and it matters only if some
caller is holding a clone of the client at that moment.

Suggested fix: before aborting, take the client out of the previous handle's status and pass it to `retire_withdrawn`,
as `stop_actor` does.

User-visible consequence: after an actor crash followed by a retry or reconcile, a stale connection can linger and
answer agent requests on that host's behalf.
