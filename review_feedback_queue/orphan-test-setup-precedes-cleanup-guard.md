# Orphan test setup precedes cleanup guard

Reviewed commit: 8bd92e1336565dfc263fe5f3fa2fef413a82a87a

## TLDR

Failure while preparing orphan-test recipes can leave earlier acquired fixture state outside the test's cleanup guard.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `e2e/tests/tensorlake-tunnels.spec.ts:87-146,308-317 at 8bd92e1336565dfc263fe5f3fa2fef413a82a87a`.

Source assessment (read with the later corrections below) (review record g074/f3, event 13):

Reviewed 8bd92e1336565dfc263fe5f3fa2fef413a82a87a. TLDR: setup failure before this browser test enters its cleanup block
can leave Tensorlake account state on a stack reused by later tests; successful test execution is separate and has
normal cleanup. Exact path: (1) line 92 atomically replaces fake config.json with {}; (2) 117-120 saves test-key and
acknowledges success; (3) 124-137 awaits recipe POST and asserts success; (4) only 146 enters try. A recipe refusal or
request rejection at step 3 never reaches finally 308-317, so the acknowledged saved key remains test-key and config
stays {}. No fixture sandboxes/snapshots have been injected at that point: first model mutation/publication is 152-172
inside try. Recipe residue depends on the failure. Verified backend recipes.rs:325-358 checks invalid
inputs/owned-snapshot refusal before INSERT and uses ON CONFLICT DO NOTHING; refused create does not insert a new row or
overwrite an existing recipe. A duplicate refusal leaves any pre-existing e2e-orphan-owner row untouched, not newly
leaked by this attempt. Route :577-584 awaits the store write before constructing successful response, so if INSERT
succeeds but client response is lost, this attempt can leave its newly committed e2e-orphan-owner row as well as
test-key. If request loss precedes commit, recipe presence is unknown; do not claim it always leaks. Source establishes
possible commit-before-response ordering, not an observed transport failure. config={} is the test's intended reset and
also its normal final state, so this alone is not evidence of harmful contamination. stack-slots.ts:4-12 confirms
replacement/project workers reuse the stack; no automatic backend restoration is supplied by the evidence fixture read
in the initial pass. Suggested fix: install cleanup around the first relevant mutation, track resource
ownership/acknowledged or uncertain creation, and restore/clear the saved key even when recipe setup refuses; avoid
deleting another attempt's existing recipe on duplicate refusal. Suggested bucket other, confidence confirmed pre-try
key cleanup gap; recipe persistence conditional on commit before lost response. Normal recipe success reaches try and
finally; this finding claims neither normal-path leakage nor live/user-data harm. Introduced
c340166d754d3edb3de7fd538c6408948d3536c6; later assigned patch only reformats setup, history through pinned tip has no
subsequent test change. proposed_drop false; possible_cover no queue/filter coverage searched. No substantive lead
remains; dynamic failure injection not run under source-only brief.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g074/f3`; supporting records
`g037/g037-i1`. The retained private log preserves earlier wording and its corrections. This record does not claim
runtime reproduction.
