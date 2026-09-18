# A stale stall verdict destroys a replacement attachment that reuses the channel

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

Reattaching to a terminal right after detaching can get the fresh view immediately torn down with a stale "stalled"
notice left over from the old one — forcing yet another reattach, potentially in a loop.

## Details

Source: pre-pr-review-swarm, area supervisor-handlers, 2026-09-17. Confidence: definite (correctness-state-lifecycle
p3). Coordinator confirmed the arbiter shape, the matching fields, and the missing identity.

When a forwarder ends stalled, `detach_stalled` spawns an untracked arbiter that re-locks the attachments map and checks
only key + channel id + owning connection (connection.rs:1513-1515). Channel numbers are reused: if the client detaches
and reattaches the same terminal on the same channel of the same connection after the old forwarder returned but before
the arbiter acquires the lock — ordinary mutex contention, both sides queue for the same lock — the new attachment
matches all three fields. The arbiter removes it, shuts down its live forwarder, and delivers `Detached(stalled)` on the
just-established channel. The comment claims this case returns silently ("the winner is using the same channel id on the
same connection", 1516-1524), but that scenario lands in the `mine` branch, not the early return — a takeover winner
always differs in channel or connection, so the comment's case can only be detach-and-reattach, where the code kills
instead of returning. `ActiveAttach` carries no per-attachment identity (terminals.rs:1308-1326: channel, lease, notify,
task handles — no nonce).

Suggested fix: give each attachment a unique identity at install (monotonic attach sequence, or the forwarder task's
`Id`), capture it in the arbiter, and treat the occupant as mine only on identity match. Same pattern as
`stale-natural-verdict-kills-replacement-attach.md`; each arbiter is independently editable.
