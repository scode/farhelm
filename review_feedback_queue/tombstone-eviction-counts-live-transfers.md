# Tombstone eviction counts live transfers, deleting too many tombstones

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

After uploading many files on one connection, a late-finishing upload can get a generic "no upload here" error instead
of the true explanation (e.g. "your session was deleted mid-upload"), sending the user down the wrong path.

## Details

Source: pre-pr-review-swarm, area data-transfer, 2026-09-17. Confidence: definite. Five lenses agreed independently (all
five correctness lenses). Coordinator confirmed the count mismatch against the docs.

`prune_finished_uploads` bounds finished-transfer tombstones to `MAX_UPLOAD_TOMBSTONES` (32), evicting the oldest past
that count (docs at `crates/farhelm-supervisor/src/service/uploads.rs:1318-1319`). The early return correctly counts
tombstones — but the eviction takes from ALL routes: `.take(routes.len().saturating_sub(MAX_UPLOAD_TOMBSTONES))`
(1333-1336). With L live transfers, L extra tombstones are deleted (8 live + 33 tombstones evicts 9, leaving 24 instead
of the documented 32). The existing test uses only one live route, so both formulas pass it. Reachable whenever a
connection runs 33+ uploads without reusing channel numbers while transfers are live.

Suggested fix: take from the tombstone list (`.take(tombstones.len().saturating_sub(MAX_UPLOAD_TOMBSTONES))`) and extend
the test with several live routes to pin the distinction.
