# Post-delete quarantine discard hangs the response on a wedged disk

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

If the disk wedges at the very end of a session delete, the delete already worked but its confirmation never arrives —
the caller cannot tell "still working" from "lost".

## Details

Source: pre-pr-review-swarm, area data-transfer, 2026-09-17. Confidence: definite (systems p2). Coordinator confirmed
the inline await and the best-effort contract. Mildest of the unbounded-removal set: no row, no claim contention of
consequence (session gone, ids never reused), no orphaned transfer — just a hung response.

After the row is removed, the delete flow awaits `discard_quarantined` (teardown.rs:866-867), a single unbounded
`remove_dir_all` (attachments.rs:366) with no timeout. The function's own contract ("best-effort and log-only… the next
startup reconciles", 360-364) concedes the removal may legitimately not happen, yet the caller pays an unbounded wait
for an outcome it is explicitly allowed to miss.

Suggested fix: bound the wait inside `discard_quarantined` (timeout, then return and let startup reconciliation handle
the debris), or detach it from the response path as an owned background task.
