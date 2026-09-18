# Losing a sink-install race can hang an attach forever

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

In rare cases attaching to a session hangs indefinitely with no feedback and never completes or fails, so the user is
left waiting on a dead request.

## Details

Source: pre-pr-review-swarm, area supervisor-teardown, 2026-09-17. Confidence: possible, low (must lose the race AND
have tmux wedge persistently in that window; blast radius is one hung attach).

On a lost sink-install race, `ensure_session_sink` awaits the loser's `candidate.shutdown()` INLINE via
`tokio::spawn(...).await` with no timeout (`reap_competing_sink`, terminals.rs:1764-1784; call sites :2029/:2039/:2049).
That shutdown joins `run_session_sink`'s orderly path ending in `shutdown_session_sink_until_safe` (terminals.rs:731),
an explicit infinite retry loop (`loop` with no exit except `Ok`: "This retry tail therefore outlives the request that
triggered it"). If the loser's sink shutdown never succeeds (tmux wedging right after the successful open), one attach
hangs indefinitely with no feedback. Every sibling wait in `ensure_session_sink` is bounded by `sink_ready` (open,
candidate/reap waits, readiness) — this is the one await that can pin a request forever. Other same-session attaches
fail fast at `await_sink_candidates`, so only the one attach hangs.

Suggested fix: wrap the join in `tokio::time::timeout(self.timeouts.sink_ready, …)` and bail on expiry — dropping the
`JoinHandle` detaches the already-spawned task, which still settles the candidate barrier and the fail-closed `Failed`
entry, so the overlap rule and the post-loop barrier keep working; the attach then fails loudly and retryably instead of
hanging.
