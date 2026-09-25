# Respawning an actor silently hands out a new per-host write lock

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After a host's connection worker crashed and restarted, the host can be edited or removed in the middle of an install or
update that was supposed to hold such edits off.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0535-2b597e9-a2a7` (audit of Area 7, helm state) as
`F3 / COR-HOST-LOCK-SPLIT-ON-RESPAWN`, tagged **possible**. Anchors and title: `manager.rs:1613`, `manager.rs:2143`,
`manager.rs:1483-1488`, `manager.rs:2566-2591` — The per-host write lock lives on the actor handle, so respawning or
reviving an actor silently hands out a new lock

The per-host write lock from F2 is created fresh for every actor. `spawn_actor` (`manager.rs:1613`) makes a new
`Arc<Mutex<()>>` and stores it on the new `ActorHandle`, and `host_write_lock` (`manager.rs:2143`) looks it up through
whichever handle is currently in the actor map. Two paths replace a host's handle while the host still exists:

- `sync_registry` respawns an actor it considers dead (retired, task finished, or nudge channel closed) and installs a
  new handle (`manager.rs:1483-1488`).
- `revive` does the same when a retry targets a retired actor (`manager.rs:2566-2591`).

Anyone still holding a guard on the _old_ lock stops excluding anyone else. That includes a provisioning run, an
in-progress `set_destination`, or a `remove_host`. The next `host_write_lock` call, and every refresh and write-back on
the new actor, uses the new, unlocked mutex. Provisioning can cause this itself: its attach step calls
`retry_now_with_fresh_window`, and if the actor has retired, that goes through `revive` and installs a new lock mid-run.

This quietly breaks the serialisation promised in `manager.rs:2128-2141` and `hosts.rs:541-552`, which says a retarget
or remove cannot move the registry under a frozen provisioning plan. It fails exactly on the failure paths. The trigger
is an actor that died, which in practice means it panicked: an actor whose row was deleted also retires, but then there
is no row to revive.

Suggested fix: keep per-host locks in a manager-level map keyed by `HostId`, create each on first use, drop it when the
host is removed, and have both `spawn_actor` and `host_write_lock` take the lock from that map.

User-visible consequence: after a host's connection worker has crashed and restarted, the host can be edited or removed
in the middle of an install or update that was supposed to hold such edits off.
