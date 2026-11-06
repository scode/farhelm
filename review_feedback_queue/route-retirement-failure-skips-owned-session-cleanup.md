# Route retirement failure skips owned session cleanup

Reviewed commit: 3650a8e1d0afc94a1438c1db09ecc08c1d5edf96

## TLDR

A route-retirement exception can prevent cleanup of sessions the test already owns.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `e2e/tests/tensorlake-lifecycle.spec.ts:299-303,341-345 at 3650a8e1d0afc94a1438c1db09ecc08c1d5edf96`.

Source assessment (read with the later corrections below) (review record g038/c5, event 14):

Discovery pass 2 interface qualification; not a new finding. Reviewed commit: 3650a8e1d0afc94a1438c1db09ecc08c1d5edf96.
TLDR: if removing the activity route throws after a page/browser closure, the changed test teardown skips deleting its
real remote session. Both finally blocks await stopActivity before cleanupSession without independent cleanup
protection. drained-route.ts:39-42 drains tracked callbacks with allSettled and then awaits page.unroute; afterEach at
lifecycle.spec.ts:137-143 drains routes but does not recover session deletion. Source evidence remains the exact locked
playwright-core 1.64.0 (e2e/package-lock.json:58-61, matching supplied read-only dependency package.json). Its
lib/coreBundle.js page unroute :63278-63300 sends interception patterns, _wrapApiCall :58881-58917 rethrows, missing
dispatcher :20140-20142 returns TargetClosedError, and closed connection :65381-65385 rejects. The separately created
session ID is already stored here (test :257/:313; helper fleet.ts:398-425); cleanupSession :561-570 would stop/delete
it if reached. Suggested fix remains nested finally or independent cleanup steps, preserving route retirement before
session deletion. Suggested bucket other: conditional maintainer-test session/resource contamination, without a
production/security/user-work-loss or catastrophic tooling route. Confidence confirmed conditional source path; no
runtime browser closure reproduction, observed leak or incidence claim. Source/helper diff through pinned tip
8a8c9579e04299410652f0e4681975a7198dfc7b was empty. Compared only the supplied interface g035:d3@21, reviewed at
8a7f66ede90651d47d1facc1e877ce81f842af1a and retained at pinned-tip test :495-505: that owner reports a different
resume-launch test asserting the token before storing the successful response's session ID. Its failure loses the
cleanup identifier; c5 retains the identifier but never reaches cleanup because route retirement throws. Neither fix
necessarily fixes the other, so keep the exact owners and mechanisms distinct despite the shared eventual session-leak
consequence. No additional assertion-path finding emitted and no other group's inventory inspected. proposed_drop false;
possible_cover unknown beyond this supplied distinct interface, coverage sources not searched. Eventual stack teardown
may reclaim leaked fixture work; this does not guarantee per-test cleanup or prevent intervening contamination.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g038/c5`; supporting records
`g035/d5`. The retained private log preserves earlier wording and its corrections. This record does not claim runtime
reproduction.
