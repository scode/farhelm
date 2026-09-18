# A stale natural-end verdict destroys a replacement attachment that reuses the channel

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

Reattaching to a live terminal right after detaching can get the fresh view torn down with a stale "terminal ended"
notice — for a terminal that never ended — forcing yet another reattach.

## Details

Source: pre-pr-review-swarm, area supervisor-handlers, 2026-09-17. Confidence: definite (correctness-state-lifecycle
p3). Coordinator confirmed the identical arbiter shape and check.

`detach_naturally` — the arbiter for `TerminalEnded` and `StreamFailed` verdicts — uses the identical spawned-task shape
and key + channel + same-connection check (connection.rs:1578-1580) as the stall arbiter
(`stale-stall-verdict-kills-replacement-attach.md`). Same interleaving: old forwarder returns, arbiter waits on the
attachments lock (or the test-seam gate above it, 1574-1576), client detaches and reattaches the same channel on the
same connection, arbiter removes the new attachment, shuts down its forwarder, and reports "session terminal ended" (or
"output stream failed") for a live terminal.

Suggested fix: same as the sibling, applied independently — capture a per-attachment identity at spawn and compare
occupant identity rather than channel plus connection alone.
