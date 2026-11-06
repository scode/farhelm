# Shift range uses refreshed row order unresolved

Reviewed commit: a9c7d4da4d043329249a9d99a82bd7699f4ba024

## TLDR

A Shift range can use a newer row order than the earlier anchor interaction. A consequential sequence beyond the
accepted short delivery interval remains unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: other.

Source location: `crates/farhelm-ui/src/list/row.rs:1776 at a9c7d4da4d043329249a9d99a82bd7699f4ba024`.

Source assessment (read with the later corrections below) (review record g086/f1, event 19):

Reviewed commit: a9c7d4da4d043329249a9d99a82bd7699f4ba024. TLDR: Shift-click handled while a same-client pin move
completes can mark a different range from the order visible at the click. Suggested bucket other; no destructive-consent
or security consequence claimed. Confidence likely for source-established ordinary selection route; unverified after
investigation for its extent outside the explicit timing exception. proposed_drop:false. possible_cover: g087:l1@10 is
an acknowledged duplicate of this ID; SPEC.md One GUI at a time:4293-4296 accepts occurrences inside its stated
sub-second window, not every asynchronous delivery gap. This corrects my prior f1 candidate's overbroad exclusion: no
inspected source bounds desktop mutation delivery to one second, so full trigger/consequence/scope coverage was not
established. Mechanism and actual checks: row.rs:1776 passes clicked Session/Gesture, while view.rs:3117-3127 reads live
listing/order_hold/selected and reconstructs the range. The same-client row-menu Move pin producer view.rs:3477-3490
sends an asynchronous request without a row_pending claim and rereads on success. Original pins A,B,C with A open may
become B,A,C; Shift-click C from the earlier display then yields A,C instead of A,B,C. rows.rs:465-488/:519-555
deliberately applies original pins' current manual order within held slots, so the hold does not freeze that range.
commit_listing view.rs:1773-1791 updates hold/listing. Exact locked dioxus-desktop 0.7.10 webview.rs:537-588 renders
state and sends separately applied edits; edits.rs:54-70 tracks their later acknowledgment. webview.rs:43-91/:190-194
dispatches events directly without waiting for the edit gate, while locked dioxus-interpreter-js 0.7.10
native.ts:352-381/:565-580 sends synchronous DOM event POSTs. These inspected primary sources establish a real producer
and an old-DOM/new-state interleaving, not an invented generic reorder. SPEC.md Session list:1792-1798 gives the
inclusive displayed-range contract; :1993-2006 permits manual pin updates during holds. SPEC_impl.md Answers
bind:1036-1047 governs question/confirmation answers and is not this finding's basis. Destructive-action retargeting and
ordinary new-session insertion remain independently refuted; later drawn actions bind their own targets, and ordinary
new rows append under a hold. Exact unresolved premise: wall-clock duration of a concrete native occurrence and
interpretation of 'as delivered' when a click reconstructs an undrawn range. Readable sources establish no one-second
bound, but do not show a concrete occurrence outside it. A native trace of Move completion, DOM mutation acknowledgment,
input delivery, resulting marked IDs and elapsed time would settle that extent. No such measurement is
permitted/requested here, and unchanged unavailable evidence was not re-investigated. Temporal check already established
live computation persists at tip 8a8c9579e04299410652f0e4681975a7198dfc7b view.rs:3232-3242. Fix direction: bind range
order/anchor to the drawn gesture, retaining live lock and disappearance guards. Neighbor preserved: supplied g087:l1@10
supersedes its earlier no-producer withdrawal, agrees on this same-client route, retains exactly this
timing/interpretation uncertainty, and proposes no drop. Parent may consolidate as duplicate with the prior disagreement
visible in this append-only history.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g086/f1`; supporting records
`g087/l1`, `g083/c8`, `g085/c4`, `g088/x3`. The retained private log preserves earlier wording and its corrections. This
record does not claim runtime reproduction.
