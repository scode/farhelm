# Re-discovering a healthy host drops its terminals

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Re-adding a host that is already connected (e.g. typing its destination into the add-host dialog again) disconnects
every terminal open on that host.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0602-2b597e9-f96c` (audit of Area 8, provisioning) as
`F16 / COR-PROBE-FORCES-RECONNECT`, tagged **possible**. Anchors and title: `provisioning/service.rs:352-363`,
`provisioning/service.rs:537-544` — Discovering an already-registered healthy host forces a full reconnect that drops
its open terminals

When a probe finds an answering supervisor, `probe()` calls `register()` (service.rs:352-363), and `register()` always
ends by calling `manager.retry_now(host)` (service.rs:537-544). `retry_now` is documented as a real reconnect: it drops
the host's current connection and dials again from scratch — even when the row already existed and nothing changed. Open
terminal streams are bound to the specific supervisor client they were attached through (`terminal.rs`,
`attach_from_query`), so every terminal open on that host detaches.

This is reached by typing an already-registered destination into the add-host dialog, or by rerunning a failed ADD for a
host whose supervisor has since come up. The same `register()` call also rewrites the row's paths without taking
`host_write_lock`, so a probe that runs while an UPDATE is in flight can move the registry row under that run (the same
invariant as F7). Discovering a host that is already fine should not disturb live sessions, and `sync_registry` already
handles genuine coordinate changes.

Suggested change: call `retry_now` only when the row was inserted or its dial coordinates or identity actually changed;
and take the host lock, or check `busy`, before rewriting an existing row.

User-visible consequence: re-adding a host that is already connected (for example typing its destination into the
add-host dialog again) disconnects every terminal open on that host.
