# Bulk test preference latch bypasses finally

Reviewed commit: a2f38b21870d0c705defc81fc5e53b6f13a3ae2a

## TLDR

An unresolved preference latch can outlive the test timeout without unwinding its finally block, skipping owned session
and preference cleanup.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `e2e/tests/bulk-session-actions.spec.ts:1134 at a2f38b21870d0c705defc81fc5e53b6f13a3ae2a`.

Source assessment (read with the later corrections below) (review record g089/c3, event 13):

Discovery revision promotes the earlier candidate using newly available exact-version primary sources; preserves the
same ID. Reviewed commit a2f38b21870d0c705defc81fc5e53b6f13a3ae2a; assigned test unchanged through pinned tip
8a8c9579e04299410652f0e4681975a7198dfc7b. TLDR: if the automatic-selection request this test is waiting for never
occurs, the test times out without running its owned session/preference cleanup, leaving state behind on the browser
suite's shared stack. Mechanism: test:1125-1129 resolves selectionReached only on GET /api/preferences; at 1134 the test
awaits the raw Promise returned by helpers/route-gate.ts:10-15. No timeout or rejection is attached to that Promise.
Cleanup of the surviving refused tabbed session and preferences exists only in the body's finally at 1153-1160. Verified
the pinned e2e/package-lock.json locks @playwright/test, playwright and playwright-core to 1.64.0 and the supplied
installed playwright/package.json and playwright-core/package.json match. In that exact runner,
playwright/lib/worker/workerProcessEntry.js:384-406 races cb() against a separate timeoutPromise; timer rejection at
425-430 does not reject the callback's latch. Worker runner:1652-1677 catches the outer timeout and starts independent
after hooks/fixture teardown. playwright/lib/index.js:398-403,425,437-447 closes the browser context independently, but
cannot resolve this host-side raw Promise or run its suspended finally. Thus the suite is bounded (NOT an
indefinite-suite hang), while test-body cleanup is bypassed on this failure boundary. Repo config:96-101 and 137-162
documents the stack reused across projects/workers; browser context teardown does not call the test's backend
cleanupSession/resetPreferences. Fix: wait for the observation through a bounded Playwright assertion or a rejecting
local deadline so its failure enters finally before test timeout; preserve releasing request gates before unrouteAll.
Suggested bucket other (test fixture isolation/reliability). Confidence confirmed for the conditional failure-path
mechanism. Caveat: no current shipped product route omitting the preference GET, actual historical occurrence or
retained runtime trace is claimed; triggering the deliberately tested automatic-selection failure is the premise, not an
established product defect. No runtime experiment was performed. proposed_drop:false; possible_cover:not searched,
parent handles coverage. This finding is distinct from duplicate g015:l5's legitimate sort-regrouping race and from
g088:c5's product task-ownership refutation.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g089/c3`. The retained private
log preserves earlier wording and its corrections. This record does not claim runtime reproduction.
