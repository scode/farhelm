# A provisioning run holds the host's cache-write lock for the whole run

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

While a host is being updated from the helm, its session list stops updating and creating, restarting, renaming or
deleting a session on it appears to hang for minutes even though the action already happened.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0535-2b597e9-a2a7` (audit of Area 7, helm state) as
`F2 / COR-PROVISIONING-HOLDS-CACHE-LOCK`, tagged **definite**. Anchors and title: `manager.rs:2143`,
`crates/farhelm-helm/src/provisioning/service.rs:821-835`, `manager.rs:1877`, `manager.rs:2074`, `manager.rs:3693`,
`manager.rs:3727` — A provisioning run holds the host's cache-write lock for the whole run, freezing its refresh and
hanging session-mutation replies

**Provisioning** is the helm's "install or update Farhelm on this host" feature. It uploads a binary, installs it,
restarts the supervisor unit and re-attaches, which can take minutes. To stop a user from retargeting the host in the
middle of a run, `start_run` (`provisioning/service.rs:821`) calls `manager.host_write_lock(host)`. It moves the
returned guard into the spawned run task (`let _host_write = host_write;` at :831) and holds it for the whole run. The
guard is held even while the task waits for a free run slot (`run_slots.acquire_owned()` at :832). An "update all"
therefore holds every host's lock while most of them queue.

`host_write_lock` (`manager.rs:2143`) is not a separate registry lock. It hands out the actor's own `cache_lock`, the
same short-lived mutex taken by:

- `remember_session` (`manager.rs:1877`), the write-back after create, restart, rename and replace;
- `forget_session` (`manager.rs:2074`), the write-back after delete;
- the refresh commit in `refresh_once` (`manager.rs:3693` for identity-less hosts, `:3727` for hosts with an identity).

While a run holds the lock, three things go wrong:

- **The refresh stalls.** `refresh_once` drains the list (bounded by `refresh_timeout`) and then waits on `cache_lock`
  with no deadline. The `serve` loop only watches for a closed connection between refreshes, not during one. So the host
  keeps showing `Connected` with a list that stops changing.
- **Mutations hang after they have already happened.** A create, restart, rename or delete on that host runs on the
  supervisor, then blocks in `record_session` / `forget_session` waiting for the lock. The HTTP reply hangs until the
  run ends. Operations relayed from agents exceed their answer budget and report "outcome unknown".
- **Retries can duplicate work.** A user or agent who retries a hung create without an idempotency key creates a second
  session.

Nothing in SPEC.md says session operations should block while a host is being updated.

Suggested fix: separate the two roles. Add a per-host registry/provisioning lock distinct from the short `cache_lock`,
and have retarget and remove take the registry lock and then the cache lock for their brief write. At minimum, bound the
refresh and write-back waits (try-lock, skip, and let the next refresh reconcile), and take the run slot before the host
lock.

User-visible consequence: while a host is being updated from the helm, its session list stops updating. Creating,
restarting, renaming or deleting a session on it appears to hang for minutes, even though the action already happened.
