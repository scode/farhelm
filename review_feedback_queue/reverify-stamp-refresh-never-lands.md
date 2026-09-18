# Reverify stamp refresh never lands, so appended sessions re-read every pass

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

Sessions whose conversation keeps growing cost the supervisor a full file read on every polling pass instead of a cheap
check, adding steady disk and CPU load that grows with the number of active sessions.

## Details

Source: pre-pr-review-swarm, area supervisor-teardown, 2026-09-17. Confidence: definite (performance only).

`CaptureState::advance` (capture.rs:312-328) refuses a same-rank `Captured`→`Captured` transition (`Captured` is not in
the 315-321 allowlist), but `reverify_capture` (capture.rs:1597-1606) relies on exactly that transition to store the
fresh stamp after a resume-append — and ignores the returned `false` (bare `.advance(...);` statement). From the first
append on, every pass re-stats, finds `differs`, re-reads the record file, and drops the new stamp: permanently one file
read per pass per appended session instead of one stat, contradicting the "an unchanged file needs no read" contract
(capture.rs:822-825: "costs one `stat` on its own record, and re-reads it only when that stamp moved"). The success path
logs nothing, so the waste is silent. No offer-correctness impact — the claim stands, per the documented rule.

To verify, append to a captured session and watch per-pass I/O (or instrument `reverify_capture`): the stamp after the
append never updates, so each subsequent pass takes the re-read path.

Suggested fix: allow same-rank `Captured`→`Captured` when the conversation is unchanged (keep refusing a different
identity), or update the stamp via a dedicated method outside the ladder; check the `advance` return at the reverify
site.
