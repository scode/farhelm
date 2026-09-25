# The seen-state queue drops a manual mark's report

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A manual "mark read" that fails at the same moment the app auto-marks the session can fail with no error shown on the
row.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as
`F22 / COR-SEEN-QUEUE-DROPS-REPORT`, tagged **possible**. Anchors and title: `crates/farhelm-ui/src/api.rs:2152`,
`crates/farhelm-ui/src/api.rs:2192`, `crates/farhelm-ui/src/api.rs:2294` — The seen-state write queue drops a newer
caller's report when the same value is re-queued during an in-flight write

"Seen" state is the per-session "you have read up to here" stamp behind the unread dot. It is written by two kinds of
caller: an automatic mark when the session view opens or its activity advances, and a manual "mark read / mark unread"
toggle in the row menu. Both go through `queue_seen_write`, which keeps at most one writer per session and passes each
caller a `report` callback that receives the final outcome. The automatic caller's report only logs a failure. The
manual toggle's report writes the row's error line, or clears it on success.

The flaw is in how the queue tracks the pending report. `SeenWrites::record` (line 2152) always overwrites `slot.report`
with the newest caller's callback. The report for the request already in flight was taken out earlier by `next_to_send`.
When the in-flight PUT settles, `finished` (line 2192) compares only values. If the newly recorded value equals the one
just sent, which is exactly what happens when a manual "mark read" and an automatic mark carry the same activity stamp,
it removes the slot and returns "not superseded". The writer (line 2294) then calls the older, in-flight report, and the
newer caller's report, still sitting in the removed slot, is dropped without ever being called. A manual toggle's
failure therefore never reaches the row, and its success never clears an earlier "seen:" error. The row-view contract
says a manual toggle's failure must surface.

The suggested fix is for `finished`, when it sees an equal value with a waiting report, to call that report with the
same result as well.
