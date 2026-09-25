# Host add/retarget/remove/adopt are not cancellation-safe

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Closing the tab or losing the connection at the moment of a host add, edit, removal or adoption can leave the host
missing, still on its old address, or stuck on a failing adoption until the helm restarts.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0535-2b597e9-a2a7` (audit of Area 7, helm state) as
`F12 / COR-HOST-EDIT-NOT-CANCEL-SAFE`, tagged **possible**. Anchors and title: `hosts.rs:486-498`, `hosts.rs:564-567`,
`hosts.rs:684-688`, `manager.rs:2375-2427` — Host add, retarget, remove and adopt commit first and are not
cancellation-safe

Each host-editing HTTP handler works in two steps: commit a change to `helm.db`, then bring the running actors in line
with it in later `await`s. When the HTTP client disconnects, axum/hyper drops the handler's future at whatever `await`
it is parked on. The store writes run in `spawn_blocking`, so the commit still completes, but the follow-up never runs.
Per handler:

- **Add** (`hosts.rs:486-498`): the row is committed but `sync_registry` never spawns its actor. The host is missing
  from `/api/hosts` (built from the actor set), is never dialed, and re-adding the same destination is refused as a
  duplicate.
- **Retarget** (`hosts.rs:564-567`): the new destination is committed but the reconcile never runs. A connected actor
  stays connected to the old address, because `serve()` does not re-read its row.
- **Remove** (`hosts.rs:684-688`): the row is deleted but `stop_actor` never runs. The actor keeps running, and if
  connected it keeps serving (see F14). Provisioning's per-host memory is also not purged.
- **Adopt** (`manager.rs:2375-2427`): the new identity is committed, but the status is never reset and the actor is
  never nudged. The host stays frozen in `IdentityMismatch`, and a second adopt fails its compare-and-swap because the
  stored identity has already moved on.

The window is one transaction plus a reconcile, so it is small. Nothing repairs the divergence on its own until an
unrelated reconcile (for add, remove and retarget), a manual retry, or a helm restart.

Suggested fix: run each commit-and-converge pair in a spawned task and await it, so a dropped request cannot split them.
Or make the runtime self-healing, for example with a slow timer that re-checks an `IdentityMismatch` against the store,
or a periodic reconcile.

User-visible consequence: closing the tab or losing the connection at the moment of a host add, edit, removal or
adoption can leave the host missing, still on its old address, or stuck on a failing adoption until the helm restarts.

Restater note: the user can recover the retarget and adopt cases without a restart. Clicking Retry on the host
(`retry_now`) nudges the actor, which then reloads its row and redials. For retarget it dials the new address. For
adopt, the next connect finds the adopted identity already recorded and connects normally. Retry does not help the add
case (no actor, so retry answers 404). For remove, retry makes the actor notice the missing row and retire, but it stays
listed until the next reconcile.
