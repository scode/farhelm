# Manual unroute leaves a listing callback running

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A test can remove a route without draining its active listing callback, letting that callback outlive assertions and
fixture deletion.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

Manual unroute does not join the new follow-up reader callback. Matching Playwright 1.64 source and the actual
reader-recovery producer establish a conditional live callback interval through later fixture deletion/context disposal.
Actual adverse scheduling is unmeasured. This is test-lifetime/oracle correctness, separately from cleanup
short-circuiting and from an admitted server write outliving the browser.

Source locations: `e2e/tests/terminal.spec.ts:2726-2735,2774-2777`. Historical revision qualifiers in those references
are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Synthesis lifetime verification, original g129/f2. In supplied Playwright 1.64 package coreBundle.js:61236-61239,
route.fetch uses browserContext.request; 60894-60961 APIResponse.json awaits body retrieval over that request context
and throws 'Response has been disposed' on missing/closed response; 64134-64143 browserContext.close disposes that
request context before closing. Worker test function/finally precedes afterEach and test-fixture teardown
(workerProcessEntry.js:1652-1677). Therefore a manually unrouted but active fetch callback has no waiting barrier before
context disposal, independently of the alternative session-absent assertion failure during fixture deletion. Source
confirms missing drain and disposal error route; occurrence still needs the documented adverse
retry/notify/late-followup schedule and was not reproduced. Stable reviewed commit
d6645f771bee8ed03a66486ef8755834937d759d; introduced f39fb5afca53044f3312ff5cac370f710fe87fe5. OTHER, confirmed
conditional source mechanism, no security/user-work route. proposed_drop null. Distinct from server-side settings-save
fencing: this callback only reads and rewrites a fixture listing.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g129/f2`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
