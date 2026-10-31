# Preference reset failure skips template cleanup

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A failed test preference reset can skip deletion of that test's owned templates.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The new template-test cleanup performs the fallible preference operation before subsequent owned-template deletion,
without an independent failure path for each resource. The older named cleanup decision does not cover these new
locations. Eventual stack teardown limits lifetime but does not refute the temporary fixture contamination.

Source locations: `e2e/tests/templates.spec.ts:520-523,577-580 at d6645f771bee8ed03a66486ef8755834937d759d`. Historical
revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed full commit d6645f771bee8ed03a66486ef8755834937d759d; also present in assigned commit
08aaac8a4a5bcf91726a1d3bfcfc980de4116971:e2e/tests/templates.spec.ts:352-355,409-412. TLDR: a failure during
browser-test cleanup can leave extra owned templates in the shared private test stack, polluting later tests. Both
number-binding tests reset preferences before deleting owned template names, and the deletion loop stops on its first
error. patchPreferences throws on a refused preferences PUT (e2e/tests/helpers/fleet.ts:1089-1113), and deleteTemplate
throws on non-success/non-404 (templates.spec.ts:22-25). Thus a failed reset prevents any deletion; a failed first
DELETE prevents later deletions. The test module explicitly shares its helm/catalog across specs
(templates.spec.ts:1-8). The existing cleanupOwnedSteps helper (helpers/cleanup.ts:1-20) would independently attempt
owned steps and preserve body failures. This failure path is inherited, not introduced by capitalization; source-only
review establishes conditional control flow, not runtime frequency or an observed later-test failure. Coverage checks:
searched SPEC.md, SPEC_impl.md, TODO.md, BUGS.md, full queue INDEX, TRIAGE_OUTCOMES.md and plans/queue for
template/cleanup/owned-resource terms. Planned bucket is empty; BUGS and filters have no exact match.
TRIAGE_OUTCOMES.md:14361-14386 (custom-permission-test-skips-template-cleanup.md) covers the same failure shape in
custom-permissions.spec.ts and permission-switch.spec.ts, with explicit two-finally scope; 14569-14593 covers
quick-launch.spec.ts; 14705-14726 covers earlier ownership arming in the tl: search test. None removes this cause in the
two templates.spec.ts number-binding tests. The existing failed-test-response-does-not-fence-settings-write queue item
concerns late writes after cleanup, rather than cleanup steps never attempted, so it is not duplicate coverage.
Suggested bucket other; confidence confirmed conditional source control flow; proposed_drop false; possible_cover none
found (related ledger precedents only). No security/user-work-loss/highest consequence claimed. Suggested repair
direction for parent: independent cleanup steps using existing helper, without changing label or shortcut behavior.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g041/f2`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
