# Detach timeout abandons the upstream detach

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

If the connection to the supervisor stalls while a terminal tab is closing, the closed tab can keep owning the session:
a browser that is long gone still holds the seat, blocking the next connection from taking over.

## Details

Source: pre-pr-review-swarm, area helm-agent-surface, 2026-09-17. Confidence: possible. Coordinator verified the timeout
path, the mid-send drop, and the phantom-owner effect.

`detach_bounded` (`crates/farhelm-helm/src/terminal.rs:735-745`; callers at :494, :522, :698) wraps
`client.detach(channel)` in a 5s timeout. `detach` (client.rs:2960-2966) removes the local `terminals` entry first and
then awaits `writer_tx.send(...).await` on the bounded queue — on a connection whose writer queue stays full for 5s
(wedged/slow peer; the writer's own stall bound is longer at 60s), the future is dropped mid-send: local state is
consistent (entry gone, later frames dropped as unknown) but the supervisor never receives `Detach` and keeps the
attachment live on a healthy connection — a gone browser keeps owning the session.

Possible: needs 5s of writer backpressure during teardown.

Suggested fix: split `detach` so the timeout covers only local removal plus a non-blocking enqueue attempt, falling back
to the `release_upstream` shape (a spawned task holding a sender clone that delivers `Detach` when capacity frees) on
queue-full instead of dropping the send.
