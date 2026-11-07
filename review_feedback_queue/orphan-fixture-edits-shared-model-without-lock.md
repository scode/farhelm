# Orphan fixture edits shared model without lock

Reviewed commit: 8bd92e1336565dfc263fe5f3fa2fef413a82a87a

## TLDR

The orphan browser fixture changes its shared fake model outside the synchronization used by its other readers and
writers.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `e2e/tests/tensorlake-tunnels.spec.ts:246-270,308-315 at 8bd92e1336565dfc263fe5f3fa2fef413a82a87a`.

Source assessment (read with the later corrections below) (review record g074/f6, event 19):

Reviewed commit 8bd92e1336565dfc263fe5f3fa2fef413a82a87a; test behavior introduced
c340166d754d3edb3de7fd538c6408948d3536c6. TLDR: a failed held-cleanup browser test can leave fake sandbox/snapshot
records on the reused test stack even after its finally prunes them, causing later tests to inherit stale fixtures.
Shipped test route: :246 holds delete_snapshot, :249-255 starts helm-owned cleanup and waits for Running, then :256-257
closes Settings while cleanup continues. An assertion failure while that request is pending enters finally; :309 resets
config to {}, releasing the hold, and :310-315 directly reads/filters/renames state.json without settling the admitted
cleanup first. Source investigation: fake tensorlake/e2e.rs:487-511 re-reads config until the hold is removed; :612-615
then enters change_model. change_model :468-474 locks only an in-process fake state_write mutex, reads the WHOLE model,
mutates it and asynchronously writes/renames the whole saved model. The spec's putModel :112-115 bypasses that mutex.
Thus a released deletion can read the old model, the spec can prune all four IDs and their snapshots, and the fake can
then rename its old whole-model copy (minus one deleted snapshot), restoring the other records. delete_snapshot's
deletion-only mutation at e2e.rs:343-349 does not prevent resurrection, because the write serializes all other old
entries too. Exact operation remains owned after Settings closes by backend orphans.rs:269-279 run_owned; this is the
very lifetime the test deliberately exercises. An alternative failure before Running observation can leave the same
admitted task pending. This is an established source-level race under the failure path, not an observed runtime failure
or normal-success failure. Normal path waits for failed/successful cleanup before final pruning, so do not claim routine
success contamination. Suggested fix: after releasing any injected hold, settle the test-owned cleanup operation/request
before directly rewriting the fake model, with bounded failure diagnostics; then apply independent cleanup attempts as
f5 requires. Suggested bucket other, test isolation/diagnostic impact only; no product-user/live resource or highest
claim. Confidence confirmed competing whole-model writes and reachable pending-operation cleanup boundary; precise
interleaving frequency not measured, no tests run. proposed_drop false; possible_cover none in supplied records; g037
cleanup sequencing concerns independent releases, not a released operation overwriting this model. Temporal tip file
history previously checked shows no later test edits. Discovery count now two new verified affected-site findings
(f5,f6), zero newly investigated-unverified findings. No substantive lead remains for this mechanism.

Later qualification (review record g074/f6, event 21):

Supplement to f6's complete mechanism record, not a new count. Reviewed 8bd92e1336565dfc263fe5f3fa2fef413a82a87a. TLDR
unchanged: failure cleanup can restore pruned fake fixture records because it releases the held backend writer and
rewrites the whole model without waiting. Further checks: e2e.rs module contract :24-26 says state.json belongs to the
fake and a spec may seed it BEFORE acting, rather than concurrently with its operation; pinned SPEC_impl.md:5812-5822
likewise assigns mutable sandbox/snapshot model to the fake. Pinned tip e2e.rs:464-481 still contains the
read-whole-model/write-whole-model sequence and private fake mutex; git log for this fake and assigned test from
introduction through tip lists only 8bd92e1, which reformats the test and does not repair model coordination. Rechecked
backend :179-243 confirms the explicit operation is Running across the awaited sweep and snapshot delete, then
transitions only on result. The latest test source retains finally's hold release and direct rewrite. Suggested bucket
other; confidence confirmed conditional source race, not dynamically reproduced, with occurrence frequency unmeasured.
Normal successful-path waits remain distinct. proposed_drop false; possible_cover none in supplied interface records
(g037 is independent-release handling; f5 records that analogous site separately). Remedy and exact competing write
schedule are in preceding f6 event; settle the owned operation before model pruning and preserve independent cleanup
attempts. No remaining substantive premise needs an unavailable source or dynamic check to state this conditional race.
Discovery totals remain two new verified findings f5/f6, zero newly investigated-unverified findings.

Later qualification (review record g074/f6, event 28):

Selected supplied g052:l5@25 is interface input, not a new g074 discovery. Its root is cancellation of a fake API future
releasing state_write while Tokio filesystem work continues and later fake transactions reuse state.json.next. Existing
g074:f6 instead releases an injected hold while a helm-owned orphan cleanup is still admitted, then the SPEC directly
read/modify/renames state.json outside the fake mutex. That race requires neither cancellation of the API future nor
shared .next names: a still-live fake transaction can rename an old whole-model copy after spec pruning even with a
unique temp file and cancellation-safe mutex. Therefore retain both exact owners and mechanisms; g052's fix for fake
transaction ownership alone would not synchronize this spec-side writer, and f6's settling-before-prune fix does not by
itself make fake cancellation safe. f6 TLDR, bucket other, source-confirmed conditional test contamination and no
dynamic reproduction remain unchanged. proposed_drop false; possible_cover related g052:l5 but not exact duplicate. No
additional discovery count or broad fake inventory inspection; supplied evidence sufficient to delimit the interface. No
live/cloud behavior assumed.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g074/f6`; supporting records
`g052/c6`. The retained private log preserves earlier wording and its corrections. This record does not claim runtime
reproduction.
