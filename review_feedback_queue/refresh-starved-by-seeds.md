# Steady session activity can starve a host's cache refresh

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

On a busy host, other sessions' status dots and titles can stop updating while the host still looks healthy.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0535-2b597e9-a2a7` (audit of Area 7, helm state) as
`F7 / COR-REFRESH-STARVED-BY-SEEDS`, tagged **possible**. Anchors and title: `manager.rs:3728`, `manager.rs:3694`,
`manager.rs:2037` — Steady session activity through the helm can stop a host's cache refresh while it reports healthy

`refresh_once` samples `seed_epoch` before the drain. If any write-back bumped it during the drain, the whole drained
list is discarded, because it predates the write-back (`manager.rs:3694` identity-less, `:3728` durable). The refresh is
still reported as `RefreshHealth::Ok`, and `serve()` then sleeps a full refresh interval (3 s) before trying again.
Nothing retries sooner: `remember_session` only asks for an immediate refresh when the reply's status is `Unknown`
(around `manager.rs:2037`).

A drain is one `ListSessions` call, and the supervisor runs its conversation-capture sweep inside that call. On a busy
host a drain can take seconds. If any helm-driven mutation on that host (create, rename, restart, delete, for example by
agents) lands during every drain, no refresh ever commits.

The session list shown to users is served from this cache. The mutated sessions stay current through their own
write-backs, but every _other_ session on that host goes stale: status dots, titles, activity. Meanwhile the host still
shows a healthy refresh.

Suggested fix: after a skipped refresh, drain again immediately (with a bound), or merge in the rows seeded since
`epoch_before` rather than discarding the drain. At minimum, do not report a skipped refresh as `Ok`.

User-visible consequence: on a busy host, other sessions' status dots and titles can stop updating while the host still
looks healthy.
