# Bulk pacing test samples before quiet regrouping

Reviewed commit: 2d46033f3f3e89e97bbb9f8e110d8e5a93a20a7b

## TLDR

The bulk Stop pacing test can compare requests with an earlier row order that its own quiet fixtures may legitimately
change before Stop is accepted.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `e2e/tests/bulk-session-actions.spec.ts:976 at 2d46033f3f3e89e97bbb9f8e110d8e5a93a20a7b`.

Source assessment (read with the later corrections below) (review record g015/l5, event 31):

Final synthesis revision. Reviewed commit 2d46033f3f3e89e97bbb9f8e110d8e5a93a20a7b. TLDR: the bulk Stop pacing check can
compare requests against an earlier displayed order even though its own quiet fake sessions may legitimately regroup
before Stop is accepted. Actual fixture: helpers/fleet.ts:169,398-426 launches fake-agent --script basic;
fake_agent.rs:1675-1689 emits startup marker/prompt and then waits for input. establishStarted at
bulk-session-actions.spec.ts:939-946 requires revealed READY and a Running/Waiting/Idle badge, but not all-Idle
convergence, settled feed readers, a held read gate or stable sort. live_status at service/status.rs:419-425 means
Running is a real sampled Working reading (not provisional, which maps Unknown); it still may decay to Idle after quiet
samples. ticker.rs:1731-1767 documents three-sample quiet settling and emits a session-change hint on the status change.
aggregate.rs:375-386,403-414 groups fresh Running/Waiting above Idle before applying the preserved work-start key. This
permits same-host owned rows to exchange relative groups as their initial readings settle; continuous fake output,
last_activity_at aging and unrelated user writes are unnecessary and are not claimed. resetPreferences at
fleet.ts:1132-1153 sets list_sort:null (default activity), and pinAutoSelect:1169-1170 only sets selection, not row
pinning or order. The test samples drawn at 976, awaits route setup at 982-998, clicks toolbar Stop at 999, and compares
per-host requests to drawn at 1013-1016. markRows:682-684 leaves the pointer over the row list; moving to Stop leaves
PinDragList (.session-list, pin_drag.rs:63-81). view.rs:3365-3372 handles pointerleave by clearing pointer_inside and
synchronously requesting hold release; rows.rs:671-677 releases unless a row menu or bell is open, neither opened by
this case. A served regrouping can already be retained under the hold (view.rs:1811-1827); rows.rs:523-525 restores its
order when the hold clears. If the toolbar rerenders before click acceptance, marking.rs:316-318 carries that newer
drawn vector. Both admission filters (view.rs:3675 and lifecycle.rs:182-192) shrink but preserve that vector's order,
and run_paced:74-96 preserves order within each host queue. Thus no product ordering failure is needed for disagreement
with the pre-leave sample. Suggested fix: explicit Created/Title sort for this pacing fixture, or bind the comparison to
the accepted snapshot; keep per-host and cross-host concurrency checks. Bucket other; confidence confirmed for the
missing barrier and permitted source mechanism. Historical failed-run attribution is unverified after investigation: no
retained trace here shows its sampled status groups, accepted toolbar vector or render timing. Those observations would
distinguish this fixture race from a product defect. It is not claimed every run reorders or that pointerleave always
renders before click. proposed_drop:false; possible_cover: bulk UI/test owner, parent deduplicates. No later edit to
this test/lifecycle/marking code occurs through pinned tip 8a8c9579e04299410652f0e4681975a7198dfc7b. No
security/data-loss route established.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g015/l5`; supporting records
`g002/g015:l5`, `g089/d1`, `g088/x2`. The retained private log preserves earlier wording and its corrections. This
record does not claim runtime reproduction.
