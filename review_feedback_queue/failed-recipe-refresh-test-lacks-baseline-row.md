# Failed recipe refresh test lacks baseline row

Reviewed commit: 68790603037bb51ccd8655e4a948c7ffb747760a

## TLDR

The failed-refresh regression does not establish the prior recipe row needed to prove that a failed refresh preserves
it.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `e2e/tests/tensorlake-recipes.spec.ts:260-293 at 68790603037bb51ccd8655e4a948c7ffb747760a`.

Source assessment (read with the later corrections below) (review record g065/r9, event 38):

Reviewed full hash 68790603037bb51ccd8655e4a948c7ffb747760a; temporal persistence directly confirmed at
744ea4122d033ef616454fe592ecb2173593736c:270-310 and pinned 8a8c9579e04299410652f0e4681975a7198dfc7b:856-912. TLDR: the
browser test documents preserving already displayed recipes during failed refresh, but passes without ever establishing
or checking such a row; clearing the loaded list could regress unnoticed. Introduction lines261-267 establish only NEW
name absence and successful initial load,271-278 force later browser GET failure,280-286 create the one owned recipe
AFTER failure routing,289 asserts that NEW row is absent,290-293 Retry makes it visible. No existing row is
created/asserted before failure or checked during the alert. Pinned lines857-869 still only establish new-name
absence/loading/error state;882-894 create after failure,900-907 check only new-name absence/retry success. A broken Err
branch retaining Some(empty_vec) rather than cached rows would still produce required stale-list wording (original UI
recipes.rs152-155, tip389-391), omit the new name, and recover on Retry. All assertions could hold while every old row
disappeared. Qualification: setting rows=None can change error wording and fail this test; claim is specifically failure
to distinguish retention from loaded-but-empty rows, not every imaginable clear variant. Governing SPEC.md303-305 at
introducing revision says failed refresh keeps shown rows; test doc251-259 (tip848-855) explicitly claims this scenario.
Supplied test-authoring checklist requires fixture premise and an observable distinguishing competing mechanisms.
Suggested fix: create an owned baseline recipe first, wait for its row in list, then inject failure/create a second
recipe and assert baseline stays visible through error and successful Retry, cleaning both. Suggested bucket other,
confidence source-verified test coverage/claim gap, no runtime reproduction or source mutation performed. No current
production row-clearing or user-owned data loss asserted. proposed_drop false; possible_cover unknown, parent handles
coverage. Distinct from refuted r6 exact-wire-expectation compatibility and r8 order-normalized stale-warning candidate.
Assigned scope remains this initial e2e patch and its later same-file context; no other-group inventory examined.
Discovery pass3 verified novel ID r9; no remaining r9 lead/premise.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g065/r9`. The retained private
log preserves earlier wording and its corrections. This record does not claim runtime reproduction.
