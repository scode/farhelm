# A retried create resurrects an archived session and launches a stood-down agent

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

Archive a session that never finished launching, and a routine automatic retry can silently bring it back to life
running a brand-new agent — undoing the user's explicit "stand this down" with no new action from anyone.

## Details

Source: pre-pr-review-swarm, area supervisor-lifecycle, 2026-09-17. Confidence: definite (correctness-systems p2).
Coordinator confirmed the archive path, all three retry gates, and the takeover predicate.

The intermediate state is stable, so no race is needed. A keyed create that never launched leaves a `Launching` row with
a `Pending` reservation. Archive requires only a map entry (handlers.rs:1341-1368, no outcome/terminal precondition),
explicitly handles terminal-less entries via the durable tmux name (teardown.rs:169-189), and `store.archive_session`
(store.rs:3644) has no outcome precondition and settles nothing — so it commits `archived = 1` while the reservation
stays `Pending` (a supervisor restart rebuilds a map entry for the `Launching` row, keeping it archivable indefinitely).
On retry, all three gates ignore the flag: `reserved_launch_evidence` sees no evidence (no pane; archive removed the
artifacts), `validate_retry` reads the full row including `archived` and proceeds (core.rs:6117-6187, no archived
check), and the takeover transaction's predicate covers pane/outcome/conversation_source only — `archived` is neither
selected nor tested (store.rs:2766-2840) — before deleting the archived row and reinserting it with hardcoded
`archived: false` (core.rs:6802). The retry then spawns an agent and publishes a live entry. This violates the stated
contract "restart is the only operation allowed to clear the flag and create a new terminal generation"
(core.rs:4906-4909).

Suggested fix: refuse the retry when the reserved row is archived, with a `Conflict` naming the archive and pointing at
restart (the sanctioned un-archive path) or a fresh intent key — atomically against a concurrent archive (hold the
lifecycle claim from validation through takeover, or add `archived` to the takeover predicate with a distinct outcome).
Do not route through `record_refused_create`'s retry arm, which deletes the row: the archived row must be kept, settling
`Failed` without deleting at most, so later retries replay the same answer.
