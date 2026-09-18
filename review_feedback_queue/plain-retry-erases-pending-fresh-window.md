# Plain retry downgrades a pending fast reconnect to a slow probe

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

Pressing retry just after editing a host can slow its reconnection — the pending fast reconnect ladder is downgraded to
a single probe with a 45-second wait.

## Details

Source: pre-pr-review-swarm, area helm-fleet, 2026-09-17. Confidence: possible, low. Coordinator verified, with a
corrected fix (the reviewer's bare `|=` is wrong).

`nudge_now` OVERWRITES `nudge.fresh_window` with the new request's value (manager.rs:2442-2449), and `watch` collapses
co-pending nudges into the latest value — so a plain `retry_now` (`fresh_window=false`) landing after a
reconfigure/attach nudge (`true`) but before the actor consumes it downgrades the merged request to a single probe:
single probe plus 45s re-probe wait instead of the ladder, self-healing at the next tick. Trigger: retry arriving in the
ms between a reconfigure/attach publish and the actor's consumption (wider under load).

The author's choice is DELIBERATE, not an oversight — the comment (:2444-2448) explains the value is retained between
sends so a previous reconfigure's flag would otherwise still ride along — but it addresses only retained-CONSUMED-true,
not pending-UNCONSUMED-true being clobbered: fresh-window-ness is monotonic evidence (a pending edit means "the host is
coming back") until consumed. Coordinator correction to the reviewer's suggested fix: bare
`nudge.fresh_window |= fresh_window` is WRONG — it reintroduces exactly the retained-stale bug the comment guards
against. Possible, low (minor + self-healing either direction; value is naming the unaddressed case).

Suggested fix: the honest fix needs edge/version semantics (e.g. a fresh-window counter, or consume-and-clear on read)
so pending-true survives a later false but consumed-true does not ride along.
