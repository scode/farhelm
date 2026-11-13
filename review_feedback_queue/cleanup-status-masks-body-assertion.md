# Cleanup checks failure status before the test has recorded it

Reviewed commit: 6e423e30068b62c6cfa276c6d720ee35bb70387a

## TLDR

If a browser assertion and its cleanup both fail, these tests can report the cleanup error as the primary failure
instead of the assertion that caused teardown. The assertion still exists in step/trace evidence; this is not total
evidence loss.

## Details

Suggested and recorder bucket: **other**. Confidence: **verified conditional source timing**, including matching locked
dependency source. No runtime failure injection ran. Reviewed change: `112fd9d812e95df7f69f66c85820d76faaf89582`;
relevant test, helper and lock source remain unchanged at the reviewed tip.

`e2e/tests/nonblocking-launch.spec.ts:215-227,331-334,441-444,556-559,636-650,790-800,889-892,960-963,1024-1027` passes
`testInfo.status !== testInfo.expectedStatus` to `cleanupOwnedSteps` from the test callback's own finally.
`helpers/cleanup.ts:15-18` logs cleanup failures when that flag is true, but throws their AggregateError otherwise.

Further reviewed sites at the pinned tip are `nonblocking-launch.spec.ts:1180-1182,1328,1426,1540,1608,1680,1771,1876`,
`launch-progress.spec.ts:213` and `tensorlake-sandbox-sessions.spec.ts:257,495`. The latter pending-Resume callback was
changed in `bd2fdb7ff1afd711fa4384a8c1d14c5d67f38fc7` (its own-finally flag was originally at `399`); the ordinary
launch callback and other added sites were reviewed in `112fd9d812e95df7f69f66c85820d76faaf89582`. These are additional
call sites of the same reporting branch, not independent defects.

Locked Playwright 1.64.0 records a hard assertion's failed expect step and rethrows before changing test status
(`package/lib/matchers/expect.js:13327-13352`). Its worker awaits the whole callback, including finally, before the
outer `_runWithTimeout` catch records the thrown test error
(`package/lib/worker/workerProcessEntry.js:986-1058, 1101-1118,1130-1148,1652-1655`). A normal hard assertion with no
earlier failure therefore still supplies false while cleanup runs. If cleanup also fails, JavaScript finally semantics
replace the propagating assertion with the cleanup AggregateError. The helper's intended primary-error preservation is
not achieved.

Catch and record whether the body threw before entering cleanup, or keep a local failure flag true until normal body
completion. Preserve expected-failure semantics deliberately. Soft assertion failure, timeout or another already
recorded error may set status early enough; those are not the confirmed route. afterEach runs after the callback's
failure boundary and is not defective merely for reading the same status property.

The fixture wrapper does not supply an earlier failure-status boundary: `helpers/evidence.ts:1-19` imports/re-exports
ordinary Playwright `expect`, and its fixture extension (`46-130`) does not catch the body's hard assertion before its
finally. Normal cleanup success still lets that original assertion escape normally. Existing correct release/drain and
HTTP-publication checks at these sites remain valid; they do not settle which error cleanup reports when it also fails.

The dependency investigation used a cached tarball matching the lock's SHA-512 integrity, without installation,
extraction/execution or network. The retained log records its full integrity/digest and exact checks. Related queue
items concern skipped cleanup resources rather than this primary-error replacement. No exact specification acceptance,
Planned item, pending repair, known bug, queue or ledger cover was found. The rare-diagnostic filter does not clearly
apply: one API outage can cause both the assertion and cleanup failures, so two errors need not be independent rare
faults or a narrow input race. Keep the step/trace caveat when presenting the consequence.

Provenance: trailing-review run `3033ee5b-b95a-4e16-8ecc-c4a9e727335a`, canonical `g024/l12`, with additional-site
records `g023/g023-l12`, `g025/l4`, `g026/f2` and `g015/l4`, exclusive base `0bc984b71384e7b754b55d007b53ebbeb9a076ca`.
Proposed drop: none for this source-established diagnostic defect.

Current-main check (2026-11-12, `6652f84fe3ecd9427b2631fb4bca4e06fa7fc741`): helpers/cleanup.ts:6-19 and the
nonblocking-launch.spec.ts own-finally calls are unchanged, as is launch-progress.spec.ts:213. The sandbox calls
formerly at :257,495 are now at :276,604 and still read testInfo.status before a hard exception reaches Playwright's
outer callback boundary. e2e/package-lock.json still pins 1.64.0, so the matching cached dependency analysis applies.
One API outage can cause both errors, leaving the rare-diagnostic filter match unclear. The conditional primary-error
masking remains source-confirmed; the original assertion remains in step/trace evidence, afterEach has a different
timing boundary, and cofailure incidence is unmeasured.
