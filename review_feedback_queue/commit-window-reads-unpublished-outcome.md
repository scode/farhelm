# A commit in the close-to-publish window gets the generic error

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

An upload commit that races a session delete at just the wrong microsecond is told "no upload here" instead of "your
session was deleted" — right answer a moment later, wrong answer now.

## Details

Source: pre-pr-review-swarm, area data-transfer, 2026-09-17. Confidence: possible (systems p2). Same degraded answer as
`tombstone-eviction-counts-live-transfers.md` through a different mechanism (race vs eviction) — independently editable,
hence separate.

When a transfer dies, `answer_queued_commits` closes the command channel first (uploads.rs:733 — load-bearing: close
stops new sends and lets the queue drain) but the tombstone outcome is published only later, in `run_upload`'s tail
(342) after the drain's reply sends and a mutex acquisition. A first commit arriving in that window gets its send
refused with Closed and is answered from `commit_without_upload`, which reads the outcome while it still says LIVE — so
the commit gets generic "no upload is in flight" although the transfer died session-gone. The window is normally
microseconds but stretches whenever the drain's sends park behind a slow writer. Tagged possible: hitting it needs
coincidental timing no party controls; the commit is always answered, just less truthfully. Coordinator confirmed the
close-then-drain-then-publish ordering.

Suggested fix: publish the ending reason before the drain instead of only the liveness flip after it — split
`UploadOutcome` into a reason recorded when the transfer decides its ending (`fail_upload`/`end_cancelled` both know
`session_gone`) and the live→ended flip that stays last in `run_upload` to preserve the channel-release ordering.
