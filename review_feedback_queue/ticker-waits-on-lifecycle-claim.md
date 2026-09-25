# The ticker waits on a session's lifecycle claim

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

While one session is stopped, restarted or deleted, other sessions' status indicators on that host can freeze.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0508-2b597e9-5788` (audit of Area 6, supervisor session state machine) as
`F17 / COR-TICKER-WAITS-LIFECYCLE`, tagged **possible**. Anchors and title: `service/ticker.rs:1265`,
`service/ticker.rs:971-975`, `service/ticker.rs:1225-1229`, `service/ticker.rs:875-889` — The ticker waits on a
session's lifecycle claim, so one stop/restart/delete freezes status updates for every session

The ticker is the supervisor's periodic background loop. On each tick, `sample_pass` walks every session one after
another, capturing each pane's screen to derive working/idle/waiting status, noticing exits and reaping tabs. It then
runs a capture pass (ticker.rs:875-889). When a session's screen changes from idle or waiting to running output, the
ticker records a "work started" timestamp durably through `persist_work_started` (ticker.rs:1225-1229). The same
function retries any earlier write that failed, on every pass (ticker.rs:971-975). To keep the write consistent with the
session's current launch generation, it first waits on that session's lifecycle claim (ticker.rs:1265), the same
per-session lock that Stop, Restart, Delete, rename and tab operations hold for their whole run.

The two can easily collide. When Stop or Restart sends SIGTERM, many agents print shutdown text. To the sampler that
looks like a fresh work start after idle, so the ticker calls `persist_work_started(…, true)` and waits for the claim.
The Stop or Restart holding it may take the full 5-second kill grace plus confirmation polling, and for Restart the
entire relaunch as well. Because the walk is serial and the tick is not concurrent with itself, sampling, the ticker's
exit detection, tab reaping and the tick's capture pass stop for every session on the host until that one operation
finishes. The write does not need to happen right then. The code already keeps a pending timestamp and retries it on the
next pass.

The user-visible effect is that while one session is being stopped, restarted or deleted, the status indicators of other
sessions on that host can freeze. The open premise is how long lifecycle operations hold the claim; Stop's sweep and
Restart are bounded by seconds, but Delete's teardown duration was not measured. The suggested fix is to try the claim
without waiting: add a `try_claim` to `KeyedLocks` if needed, and when the claim is busy, leave the pending timestamp
for the next pass instead of blocking the whole tick.
