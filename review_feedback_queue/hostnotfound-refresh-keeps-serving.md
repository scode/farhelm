# A connected actor keeps serving after its registry row is deleted

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

In the rare paths that delete a host without stopping its connection, the host keeps showing connected and keeps a
connection to that machine open after it was supposedly removed.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0535-2b597e9-a2a7` (audit of Area 7, helm state) as
`F14 / COR-HOSTNOTFOUND-KEEPS-SERVING`, tagged **possible**. Anchors and title: `manager.rs:3791`, `manager.rs:3810`,
`manager.rs:2897` — A connected actor whose registry row was deleted keeps serving, because a HostNotFound refresh does
not end the connection

An actor checks whether its registry row still exists only at the top of its run loop (`reload_row`, `manager.rs:2897` /
`:3857`). A `RowStatus::Removed` result retires it there. Once connected, it stays inside `serve()` and never reaches
that check again until the connection ends.

If the row is deleted while the actor is connected, every later refresh fails in `replace_host_sessions` with
`HostNotFound`. But `refresh_once` ends the connection only when the store error is `IdentityMismatch` (the "superseded"
check at `manager.rs:3791`, and `end_connection` at `:3810`). For `HostNotFound` it records a failed refresh and carries
on. The actor stays `Connected` (with a failed-refresh health), keeps its ssh transport, drains the host every interval,
and can still answer agent upcalls for a host the registry no longer has.

The normal removal path aborts the actor first, so this needs a path that deletes the row without stopping the actor.
F13 and the remove case of F12 are two such paths. The code already fixed the same shape for a superseded identity; its
comment warns that such a host "would sit there looking connected while caching nothing".

Suggested fix: treat `HostNotFound` like the superseded-identity case and set `end_connection`, so the actor returns to
its run loop, reloads the row, and retires.

User-visible consequence: in the rare paths that delete a host without stopping its connection, the host keeps showing
as connected and keeps a connection open to that machine after it was supposedly removed.

Restater note: this fix only covers hosts with an identity. An identity-less host never writes to the store during
refresh, so it never sees `HostNotFound`. It would keep serving in the same situation until the next reconcile removes
its actor.
