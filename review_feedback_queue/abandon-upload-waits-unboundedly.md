# Transfer cleanup waits unboundedly, pinning claims and blocking deletes

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

If the disk wedges while an upload is being cleaned up, the request hangs forever, the session cannot be deleted, and
its other operations pile up behind a lock nobody can release.

## Details

Source: pre-pr-review-swarm, area data-transfer, 2026-09-17. Confidence: definite. Two lenses agreed (systems p2,
data-flow p3). Coordinator confirmed the unguarded awaits, the claim hold, and the delete wait.

Every transfer ending cleans up through `abandon_upload`, which awaits `spawn_blocking` removes with no timeout, no
firing retry bound (the loop at 1229-1256 only iterates if an await returns), and no race against cancellation — the one
disk wait in the transfer lifecycle with none of `await_disk_stage`'s guards. Consequences: (1) `commit_upload`'s
refusal path calls it holding the lifecycle claim (1046/1069) and answers the commit only after it returns (1075-1084),
hanging the commit and its helm HTTP request (unbounded on both sides); (2) `fail_upload` likewise answers queued
commits only after abandoning; (3) on every other path the parked task never drops its `finished` sender, so
`abort_session_uploads` (1301-1306) blocks forever and the session can never be deleted. This contradicts `run_upload`'s
contract ("every exit path of this task is bounded by either the client, the progress timeout, or a delete", 314-316)
and inverts the function's own "cleanup-attempt-plus-backstop" promise. The adjacent comment (1087-1094) shows the
authors bounded publication for exactly this reason but missed abandon.

Suggested fix: bound each removal attempt with the existing `upload_disk_stage` timeout and, past the final attempt,
give up to the documented backstop (startup reconciliation). Giving up is already the designed semantic for failed
removals; the timeout extends it from "failed" to "never answered".
