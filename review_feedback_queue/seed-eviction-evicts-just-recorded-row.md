# New session immediately evicted from an at-capacity list

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

In a rare case, a session you were just told exists vanishes from the list immediately — it cannot be reached until the
next refresh, and the list wrongly reports itself as truncated.

## Details

Source: pre-pr-review-swarm, area helm-fleet, 2026-09-17. Confidence: possible. Coordinator verified.

When a seed pushes an identity-less host's list past `LIST_SESSIONS_CAP`, the victim is selected by `max_by` over ALL
entries INCLUDING the just-inserted session (manager.rs:1918-1931). The comment (manager.rs:1905-1917) promises eviction
"never by refusing the new row" ("a list without its row would leave a session the caller was just told exists
unroutable until the next refresh") and claims "the same choice the durable path's SQL makes" — but the SQL excludes the
new row (`session_id != ?2`, "the oldest OTHER row goes", store.rs:4444-4448).

If the new session is the minimum under (created_at ASC, id DESC) — degenerate/zero `created_at` from the peer, or ties
with the largest id — the session the caller was just told exists is evicted immediately: unroutable until the next
refresh, with a spurious `list_truncated` flag. Narrow trigger (at-cap list via seeds between drains, plus anomalous
timestamps) but a definite divergence from both the documented promise and the sibling implementation, in exactly the
outcome the promise names. Possible.

Suggested fix: exclude the new row from victim selection (skip `session.id` in the `max_by`, mirroring the SQL's
`!= ?2`).
