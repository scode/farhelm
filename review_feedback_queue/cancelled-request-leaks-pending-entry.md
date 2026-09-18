# Cancelled request leaks its pending entry

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

No direct user-visible symptom at small scale: requests cancelled while waiting for a supervisor reply leave a small
entry behind that is never freed, so a long-lived server slowly accumulates dead bookkeeping for requests nobody is
waiting on.

## Details

Source: pre-pr-review-swarm, area helm-agent-surface, 2026-09-17. Confidence: possible. Coordinator verified the insert
path, the missing removal on drop, and the narrow scope of the ordering comment.

The reserve-before-register ordering in `request()` (`crates/farhelm-helm/src/client.rs:2322-2346`) prevents orphaned
`pending` entries only for cancellation DURING the capacity wait — the comment (client.rs:2326-2335) names exactly that
case. Once `pending.map.insert(req_id, tx)` runs (:2344), dropping the future (axum handler dropped on client
disconnect, `select!` losing a race) leaves the oneshot sender in the map: removal happens only when a reply arrives
(client.rs:1699) or `fail_all` drains on connection death (:1495). The demux comment (client.rs:1690-1698) acknowledges
cancelled callers but relies on the late reply arriving to clean up — if the supervisor never answers on a still-live
connection, the entry leaks for the connection's lifetime. Slow unbounded growth of `pending.map` under repeated
cancel-without-reply (small per entry: u64 + dead oneshot sender — low severity, but precisely the orphan class the
surrounding code claims to have closed structurally).

Possible: needs cancel-without-reply on a live connection; minor leak per event.

Suggested fix: guard the `rx.await` with a scopeguard removing `req_id` from `pending.map` on early drop (disarmed on
normal reply), mirroring the check-and-insert-under-one-lock discipline.
