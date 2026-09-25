# Each open session view triggers a full host list read per fleet change

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

With sessions open in the helm, each remote host is asked for its full session list several times more often than
intended, adding load and ssh traffic and slowing detail updates on slow links.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0535-2b597e9-a2a7` (audit of Area 7, helm state) as
`F18 / COR-DETAIL-FULL-DRAIN`, tagged **definite**. Anchors and title: `sessions.rs:2428`, `sessions.rs:2517`,
`crates/farhelm-ui/src/session_view.rs:922-929` — Every fleet change makes each open session view trigger a full live
session-list read on its host

`GET /api/sessions/{id}` (`get_session`, `sessions.rs:2428`) returns one session's details. For a connected host it
calls `manager::drain_sessions(&client)` (`sessions.rs:2517`), which is a full `ListSessions`: up to 500 rows, plus the
supervisor's whole-host capture sweep. It then picks out the one row it needs. The protocol has no per-session query.
The handler's doc says the route "exists for the recovery paths rather than for browsing".

The UI calls it much more often than that. Every open session view registers a feed reader
(`crates/farhelm-ui/src/session_view.rs:922-929`) that calls `feed_detail(Trigger::Notice)` on _every_ fleet-revision
notice. The revision is bumped by any host's changed refresh, which is roughly every 3 s while an agent is working, and
by seen-marks and profile/default writes. So every open session view, in every client, causes about one extra whole-host
`ListSessions` on its host per bump, including bumps caused by other hosts. The reader allows one request in flight per
view, which bounds the rate but not the multiplication. This detail read also has no `REFRESH_TIMEOUT`, unlike the
actor's own drain.

This multiplies the supervisor-side sweep cost that the 3-second `REFRESH_INTERVAL` was sized for, and ships the whole
list over ssh to answer one row. Usually that is the row the actor has just written to the cache.

Suggested fix, per the finding: answer detail reads for a connected host from what the helm just recorded (the cache
row, or the in-memory list for identity-less hosts). Keep the live read for explicit recovery triggers, or share one
drain per host among concurrent readers. Bound any live read with `REFRESH_TIMEOUT`.

User-visible consequence: with sessions open in the helm, each remote host is asked for its full session list several
times more often than intended. That adds load and ssh traffic, and slows detail updates on slow links.

Restater note: the first suggested remedy contradicts a documented design decision. `get_session`'s docs
(`sessions.rs:2404-2410`) state that "a reachable host's detail must never come from" the cache, citing PLAN_M6: a
detail read lagging the refresh cadence would show a restart offer that no longer exists. The cost amplification is
confirmed. The remedies that do not conflict with that decision are sharing one drain per host, bounding the read with a
timeout, and changing the UI so a notice caused by an unrelated host does not trigger a detail re-read. Serving from the
cache would need that decision revisited.
