# Confirming ADD rewrites the host row before the busy check

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Confirming setup for a host that is already updating is refused as busy but still repoints the host at different paths,
so the running update can fail and the host can stop connecting.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0602-2b597e9-f96c` (audit of Area 8, provisioning) as
`F7 / COR-ADD-REWRITES-ROW-BEFORE-BUSY`, tagged **possible**. Anchors and title: `provisioning/service.rs:575-591`,
`provisioning/service.rs:811-815`, `provisioning/service.rs:503-534` — Confirming an ADD rewrites an existing host row
and forces a reconnect before the busy check and without the host lock

Confirming an ADD (`start_add`, service.rs:575-591) consumes the one-use plan and immediately calls
`register(registration, Some(&plan))`. For an existing destination that rewrites the row's coordinates to the plan's
(`register_probed_ssh_host`), reconciles the live registry, and calls `retry_now`, which drops the host's current
connection. Only after all that does `start_run` check whether the host already has a run in flight (the `busy` set,
service.rs:811-815) and take `host_write_lock`.

An ADD plan can exist for a host that is being updated: an ADD plan is minted whenever a probe finds no answering
supervisor, which also happens while an UPDATE is restarting it, or the plan may have been minted earlier and confirmed
late from a second tab or the API. Confirming it then moves the row to the ADD defaults and drops the connection, and
only then is refused with 409 Busy. The rewrite is not undone and the plan is gone. The running UPDATE's attach step,
and every later dial, now use paths that UPDATE never installed.

The `host_write_lock` documentation promises that a confirmed run's registry row cannot move underneath it; this is a
path around that promise, and a refused request that leaves a durable change behind. Suggested change: claim `busy` and
the host lock before registering an existing row (or do the registration inside the claimed run) and roll back on Busy;
or refuse ADD for an already-registered destination.

User-visible consequence: confirming setup for a host that is already updating is refused as busy but still repoints the
host at different paths, so the running update can fail and the host can stop connecting.
