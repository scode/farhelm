# Session delete strips a fresh collision record from the new connection

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

Deleting a session can briefly clear the record that two machines hold the same session id — during that window a
genuine collision is not refused when it should be.

## Details

Source: pre-pr-review-swarm, area helm-fleet, 2026-09-17. Confidence: possible, low. Coordinator verified.

`forget_session` performs two back-to-back `send_modify` calls (manager.rs:2024-2050). The first (in-memory live-list
removal) silently no-ops on incarnation mismatch or disconnected client — unlike `remember_session`'s equivalent block,
which bails. The second (contested-set removal) re-checks NOTHING: it edits whatever `contested` is currently published,
even when the first block just determined the claim is stale. A delete issued against a superseded connection (claim
predates a reconnect) therefore strips the session id from the NEW connection's freshly observed contested set and emits
a spurious `events.bump()` — deterministically given the setup, no thread race needed (the cross-task variant between
the two synchronous mutations exists too but is nanosecond-narrow).

Self-heals on the next successful drain (contested is reconstructed refresh state), so the window is one refresh
interval of fail-OPEN risk: a genuine collision temporarily unrefused (the fail-open twin of
`disconnect-publishes-keep-stale-contested-claims.md`'s fail-closed stale claims). Possible, low.

Suggested fix: fold the contested removal into the first `send_modify` under the same incarnation/client guard, and bail
on stale like `remember_session` does.
