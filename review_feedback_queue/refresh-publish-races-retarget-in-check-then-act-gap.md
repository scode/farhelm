# Retarget landing in the check-then-publish gap reverts to the old connection

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

In a microsecond-narrow race, editing a host's destination can be briefly overwritten by the old connection's status —
self-healing within microseconds, with operations failing safely in the meantime.

## Details

Source: pre-pr-review-swarm, area helm-fleet, 2026-09-17. Confidence: possible, low. Coordinator verified.

The `taken_nudge` check (:3392) and the `publish_refresh` (:3398-3408) are synchronous but NOT atomic with respect to
the manager thread: a retarget publish + nudge (manager.rs:1484-1507) landing in that ~10-instruction gap means the
actor overwrites the new row's `Connecting` with the old connection's `Connected` (old client, old incarnation lineage
broken only later). Narrower than `stale-dial-outcome-publishes-over-retarget-nudge.md` — requires landing in the gap
rather than winning a `select!`. Fails in the SAFE direction (operations on the retired client error, or their claims
fail the bumped incarnation) and self-heals at the next loop select (microseconds). Possible, low.

Suggested fix (defense in depth): after `publish_refresh`, re-check `taken_nudge` and on `Some` immediately break, so
the end-of-`serve` `Connecting` publish repairs the overwrite within microseconds instead of after a full select cycle.
