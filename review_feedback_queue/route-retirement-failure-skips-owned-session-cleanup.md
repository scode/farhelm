# Fallible teardown can skip independent owned cleanup

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

## Additional fallible teardown prerequisites

Additional reviewed commit: `6e423e30068b62c6cfa276c6d720ee35bb70387a`, introducing change
`112fd9d812e95df7f69f66c85820d76faaf89582`.

`e2e/tests/nonblocking-launch.spec.ts` has further cleanup gates outside its best-effort owned-resource boundary. These
are separate failure operations from the original `page.unroute` route, with the same consequence: later independent
cleanup steps are never attempted.

In the asynchronous-host-result test (`779-800`), the finally block releases its held replies, then evaluates and
disposes an `admissionTrace` handle (`783-784`), disposes routes and scans the session list before reaching
`cleanupOwnedSteps`. A lost page execution context after handle acquisition makes the evaluation reject. That skips
cleanup of already-owned fake sessions, templates and quick-launch preferences, although their request context is
independent of the failed page. Other blocks await unprotected route disposers before their API cleanup; those are
additional sites of the original route-retirement failure mechanism, not proof that every unroute rejects on closure.

Further route-disposal sites are `e2e/tests/launch-progress.spec.ts:212-215,328-330` and
`e2e/tests/yolo-guard.spec.ts:301-309`. They await `stop()` or `unrouteAll` before independent session, template,
preference or host-policy cleanup. The matching cached Playwright-core 1.64.0 tarball has lock-matching integrity;
`package/lib/coreBundle.js:63274-63300` forwards unroute/unrouteAll through interception-pattern updates without
suppressing closed-target failure. An absent target yields TargetClosedError (`20140-20142`), which rejects the client
command (`65439-65448`). Page loss can therefore reject disposal while its separate API request context remains usable.
These sites add to the existing route-disposal proof; they do not turn the handle-evaluation or discovery-scan
prerequisites into unroute duplicates.

The optional session-discovery scan is another prerequisite. The empty-fleet finally releases/drains at `1323-1324`,
then awaits `listSessions` at `1325`, before cleanup at `1328`. Its known session IDs were captured at `1240-1242`, and
preferences/template restoration exists only inside the later cleanup. The same order appears at `1421-1426`,
`1534-1540`, `1766-1771` and `1870-1876`. `helpers/fleet.ts:389-398` throws on request failure, non-OK response or bad
JSON. A failed discovery therefore skips cleanup that could use the IDs already known. Proper gate release before drain
remains correct; this finding narrows the earlier broader conclusion that independent cleanup would be reached.

`helpers/cleanup.ts:6-19` attempts supplied steps independently, but cannot protect work done before it is called. Make
page-handle cleanup, each route disposal and optional discovery independent fallible steps; later known-ID and
preferences/template cleanup must still run when one fails. Preserve the original test failure and use any IDs a
successful scan adds. This does not promise to recover cleanup after every network outage or a closed request context.

Suggested bucket remains **other**. Confidence for these added branches is **verified conditional source control flow**,
not observed leakage. Actual leftovers require the failed prerequisite while subsequent independent cleanup could still
work. The private stack eventually has wider reclamation, but that does not prevent intervening contamination. No
user-work loss, vendor exposure or catastrophic tooling consequence is claimed. No exact accepted limitation, Planned
item, pending repair, known bug, ledger or filter cover was established for these added branches. The existing
recipe-navigation item remains a related, separately named failure boundary.

Provenance: trailing-review run `3033ee5b-b95a-4e16-8ecc-c4a9e727335a`, `g024/l6`, `g025/c4` and `g023/g023-l6`,
exclusive base `0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Original candidate reversals, the valid gate-order refutation
and the retained cleanup finding remain in the private history. The g023 normal release/drain/HTTP-publication
refutations remain valid; this failure boundary does not revive its refuted normal-path late-create or
premature-owned-ID snapshot claims. No runtime failure injection ran.

Current-main check (2026-11-12, `6652f84fe3ecd9427b2631fb4bca4e06fa7fc741`): nonblocking-launch.spec.ts:783-790 is
unchanged: browser-handle evaluation and disposal, route retirement and optional API discovery still precede independent
known-ID, preference and template cleanup. launch-progress.spec.ts:212-215,328-330 and yolo-guard.spec.ts:301-309 retain
their route-retirement prerequisites. The existing route-retirement queue root remains listed at INDEX.md:1527-1528.
Browser-handle evaluation and optional discovery add separate fallible prerequisites under that broadened root,
preserving the original proof and all three aliases. This remains a source-confirmed conditional cleanup boundary,
without an observed browser-crash leak.
