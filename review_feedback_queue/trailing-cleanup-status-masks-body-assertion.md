# Retirement test can replace its body assertion with a cleanup error

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: When this browser regression test first fails a hard assertion and its cleanup also
fails, the reported callback error can become an AggregateError about cleanup instead of the assertion that exposed the
regression. This complicates diagnosis; assertion step/trace evidence remains, so the original assertion is not
universally lost. Source checked: new retired-container-sessions.spec.ts:114-118 reads info.status !==
info.expectedStatus inside the body's own finally and passes that value to cleanupOwnedSteps. Unchanged
helpers/cleanup.ts:6-19 attempts each step, but only logs cleanup errors when bodyFailed is true; with false it throws
AggregateError. e2e/package-lock.json:13-20,42-49,58-60 pins @playwright/test, playwright and playwright-core to 1.64.0.
Caller-supplied already-recorded dependency source evidence establishes the timing premise for that locked runner: a
hard expect records its failing step and rethrows before test status is changed; the runner awaits the complete
callback, including finally, before its outer catch records body failure. Therefore a first hard assertion can leave
info.status equal to expectedStatus in this finally; throwing cleanup AggregateError replaces the propagating assertion
under JavaScript finally semantics. This timing differs from reading status in afterEach. Contract: the helper's own
lines 3-4 promise 'An assertion failure remains the primary failure; cleanup errors accompany it in stderr.' The new
caller does not reliably provide the argument needed for that contract. Capture body failure explicitly with
catch/rethrow plus a local f

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g042/f1`.

### g042/f1

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 11 (finding, open):

Reviewed commit: 04d85f0c98a9b635709d00b056719a0baa7fd3fc. TLDR: When this browser regression test first fails a hard
assertion and its cleanup also fails, the reported callback error can become an AggregateError about cleanup instead of
the assertion that exposed the regression. This complicates diagnosis; assertion step/trace evidence remains, so the
original assertion is not universally lost. Source checked: new retired-container-sessions.spec.ts:114-118 reads
info.status !== info.expectedStatus inside the body's own finally and passes that value to cleanupOwnedSteps. Unchanged
helpers/cleanup.ts:6-19 attempts each step, but only logs cleanup errors when bodyFailed is true; with false it throws
AggregateError. e2e/package-lock.json:13-20,42-49,58-60 pins @playwright/test, playwright and playwright-core to 1.64.0.
Caller-supplied already-recorded dependency source evidence establishes the timing premise for that locked runner: a
hard expect records its failing step and rethrows before test status is changed; the runner awaits the complete
callback, including finally, before its outer catch records body failure. Therefore a first hard assertion can leave
info.status equal to expectedStatus in this finally; throwing cleanup AggregateError replaces the propagating assertion
under JavaScript finally semantics. This timing differs from reading status in afterEach. Contract: the helper's own
lines 3-4 promise 'An assertion failure remains the primary failure; cleanup errors accompany it in stderr.' The new
caller does not reliably provide the argument needed for that contract. Capture body failure explicitly with
catch/rethrow plus a local flag before finally, as appropriate for an in-body cleanup caller. Suggested bucket: other
(test failure diagnostics, no product security/work-loss route). Confidence: confirmed source analysis using locally
checked caller/helper/locked-version correspondence plus the expressly supplied runner-source premise; no runtime
observation or reproduction. proposed_drop: false. possible_cover: caller explicitly identifies existing mechanism
cleanup-status-masks-body-assertion at other call sites and will extend it with this new in-range caller; no queue
metadata read or searched. This is a linked caller finding, not a request for a second mechanism entry.
Could-be-highest: no. Caveats: requires both a body hard assertion failure and cleanup rejection; helper still attempts
later cleanup steps, and page-route separation/owned-id cleanup remain valid. This does not refute c4's narrow
presentation/dispatch coverage or establish successful real Podman removal. Attribution: this test and its in-body
status argument are introduced by the assigned commit, even though the helper already existed. No universal cleanup
success or primary-error preservation is claimed.

Existing parent disposition (event 11): verified. Parent read complete originalf1 and c4 revision, focused claims-026
and the entire existing cleanup-status-masks-body-assertion file. New04d85f0c retired-container-sessions.spec.ts:114-118
passes info.status!==info.expectedStatus from its own finally to unchanged helpers/cleanup.ts:6-19. Locally checked
lock1.64.0 matches the existing item's retained runner-source proof: first hard expect rethrows before status update,
and outer callback catch records failure after finally. Cleanup AggregateError can replace that propagating assertion,
whose step/trace still survives. This is a useful in-range caller extension of the exact existing mechanism, not a
second root or a claim of runtime failure injection. Ordinary presentation/readiness/independent cleanup attempts remain
sound; cofailure incidence unmeasured. No exact specification/Planned/unlanded-plan/BUGS/active-outcome acceptance or
complete rare filter covers the extension beyond existing root.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g042 record](records/review-through-00de0b6c/groups/g042.json). Record ID: `f1`.
