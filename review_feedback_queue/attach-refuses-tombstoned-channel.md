# Attach refuses a channel that only holds a finished upload's tombstone

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

Upload a file and then attach to a session on the same channel, and the attach is wrongly refused — the finished
upload's kept receipt blocks the channel even though nothing is still transferring.

## Details

Source: pre-pr-review-swarm, area supervisor-handlers, 2026-09-17. Confidence: definite. Two lenses agreed
(correctness-edge-inputs p1, correctness-systems p1). Coordinator confirmed the asymmetry; the restater corrected one
detail (the refusal names no session) and the coordinator verified the correction.

Each connection numbers its own channels from 1; one channel carries terminal input or an upload, tracked in
`input_routes` / `upload_routes`. Finished upload routes are deliberately kept as tombstones (up to the 32-entry cap) so
the client can retrieve the result — but the channel is meant to be reusable. `BeginUpload` checks route liveness
(`is_upload_route_live`), refuses only active transfers, and documents reuse as designed ("either use must refuse an
ACTIVE channel and may take a dead one", handlers.rs:2457-2461). `handle_attach` instead refuses on mere presence in
`upload_routes` (`crates/farhelm-supervisor/src/service/handlers.rs:1476-1478`), with "attachment channel {channel} is
already in use" (1483-1484). The frame router itself checks liveness (connection.rs:376-379), so attach is the one use
that breaks the design; its doc comment ("a channel carrying an upload counts as in use", 1456-1460) does not
distinguish a live transfer from a kept result.

Suggested fix: use the same liveness check in `handle_attach`, refusing only channels with an active route. Optionally
prune the dead route when taking the channel; do not change tombstone retention itself.
