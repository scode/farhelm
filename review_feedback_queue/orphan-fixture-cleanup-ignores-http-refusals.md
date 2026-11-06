# Orphan fixture cleanup ignores http refusals

Reviewed commit: 8bd92e1336565dfc263fe5f3fa2fef413a82a87a

## TLDR

Fixture cleanup can treat a refused HTTP operation as completed because it never checks the response status.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `e2e/tests/tensorlake-tunnels.spec.ts:316-317 at 8bd92e1336565dfc263fe5f3fa2fef413a82a87a`.

Source assessment (read with the later corrections below) (review record g074/f9, event 25):

Reviewed commit 8bd92e1336565dfc263fe5f3fa2fef413a82a87a. TLDR: this browser test can pass even though its temporary
recipe or saved test API key was not removed, leaving later tests on its reused stack to fail instead. Mechanism:
finally awaits DELETE recipe and PUT clear_api_key at :316-317 but discards both response status/body. Locked
e2e/package-lock.json:58-61 pins playwright-core 1.64.0, verified against read-only supplied dependency package.json:3.
Its matching types/types.d.ts:19547-19551 and method-option docs :19868-19872 explicitly state response objects are
returned for all HTTP statuses by default; no failOnStatusCode override occurs in the assigned calls or pinned
playwright.config.ts use options. Thus a cleanup HTTP 500 resolves normally and the test can complete successfully.
Exact non-vendor route exists: tensorlake/config.rs:138-152 performs a fallible transactional account update, propagates
SaveError::Storage at :156, and tensorlake/mod.rs:378-385 returns HTTP 500 without clearing the prior key when that
write fails. Recipe DELETE similarly returns storage failures at recipes.rs:672-675,685-691 and preserves the row if
deletion failed. This needs no claim of Tensorlake gateway policy, foreign-project status, or HTTP error caused by
vendor cleanup. e2e/tests/helpers/cleanup.ts:1-20 says cleanup failures after successful body must fail the test, and
shared-stack reuse was verified earlier. Fix: check each cleanup response with a labelled status/body assertion inside
independent cleanupOwnedSteps callbacks (combine with f5), so the failure is attributed to this fixture and later
independent cleanup is still attempted. Distinct root from f5: f5 is an exception aborting later releases; f9 is missing
failure detection even when every await resolves. Merely wrapping these unchecked awaits in cleanupOwnedSteps does not
fix f9. Suggested bucket other, maintainer-test contamination/diagnostics only. Confidence confirmed conditional source
path and version-matched request contract, not dynamically reproduced; no claim that an actual storage failure has
occurred. Normal successful cleanup statuses are unaffected. Origin
c340166d754d3edb3de7fd538c6408948d3536c6:test:160-161; later assigned patch only reformats and no later file edit
through pinned tip. proposed_drop false; possible_cover related f5 and supplied g037:g037-l1, but that record concerns
throwing/asserted recipe cleanup preventing independent clear in another fixture; this adds the unchecked-response
mechanism at this assigned site. Discovery pass 2 totals now two novel source-verified findings f8/f9, zero
investigated-unverified; lead f9 resolved.

Later qualification (review record g074/f9, event 27):

Completion evidence for f9, not an additional discovery. Reviewed behavior 8bd92e1336565dfc263fe5f3fa2fef413a82a87a;
directly read pinned tip :308-319 and confirmed both response statuses still discarded. Re-read reviewed
playwright.config.ts:115-125 confirms default request fixture use options lack failOnStatusCode; matching 1.64.0
dependency docs and pinned lock were checked before reliance, read-only. TLDR remains that HTTP failure of recipe/key
restoration can let this test pass with shared state left behind. Suggested bucket other; confidence source-confirmed
conditional return-status detection failure, not dynamic storage-failure observation. proposed_drop false;
possible_cover related f5, but missing status detection is distinct from exceptions aborting independent cleanup;
aggregate cleanup alone without assertions would still miss this case. Prior full f9 event contains route/store failure
code and fix. Discovery pass 2 completed with two new source-verified findings f8/f9, zero investigated-unverified
findings and no new exact duplicates. No unresolved f9 lead. No runtime/build/process/browser/network/live-install
action performed.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g074/f9`. The retained private
log preserves earlier wording and its corrections. This record does not claim runtime reproduction.
