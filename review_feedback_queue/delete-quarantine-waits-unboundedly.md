# Session-delete quarantine awaits the disk unboundedly under the claim

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

If the disk wedges while a session is being deleted, the delete hangs forever — and every new upload to that session
piles up behind it, hanging too, with no error ever returned.

## Details

Source: pre-pr-review-swarm, area data-transfer, 2026-09-17. Confidence: definite (systems p2). Coordinator confirmed
the unguarded awaits and the claim hold.

Delete quarantines the attachments directory (rename into `.quarantine`) before removing the row, fail-closed. The three
awaits — `metadata` (attachments.rs:331), `ensure_private_dir` (342), `rename` (348) — have no timeout and no
cancellation race, while the caller holds the lifecycle claim across the whole call (teardown.rs:829-833, abort round
already run). A wedge hangs the delete holding the claim, and every new upload admitted afterwards parks in
`stage_upload`'s claim wait (786-806) with no releasing signal, each parking its own helm HTTP request (`begin_upload`
has no timeout either). Fail-closed is right for an error, but a wedge never resolves into one: the row is retained with
no retry ever possible.

Suggested fix: wrap the quarantine awaits in a timeout inside `quarantine_session_dir`, mapping expiry to the same
`Err(String)` the I/O failures return. Failing closed with the row retained is exactly the documented semantics — a
timeout preserves the design while converting an indefinite hang into a retryable refusal and releasing the claim.
