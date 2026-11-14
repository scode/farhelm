# Synchronous-recognition test only asserts eventual callback output

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: the new test says it protects immediate file-link recognition, but would still pass if
recognition delivered the same result asynchronously; it cannot catch the scheduling regression it explicitly claims to
prevent. Source: test comment at 398-399 says 'Recognition must stay synchronous for ordinary supervisor files.
Retiring * checkout confinement must not add host Stat requests before offering a path.' The test at 400-413 installs a
request stub that throws, then line 410 awaits a Promise resolved by provideLinks and only compares the eventual paths
at 411. No assertion checks whether the callback ran before provideLinks returned. The host-lookup stub does protect the
no-request half, but both synchronous and deferred no-request callbacks satisfy the timing-blind observable. Production
provideLinks at terminal-files.js:672-677 currently calls back synchronously, so no production timing regression is
alleged; production correctness does not establish the test's stated timing oracle. The review charter's test checklist
requires an observable that distinguishes competing mechanisms, and the test itself declares the synchronous contract.
Related SPEC.md:3126 requires immediate underline/checking feedback; SPEC_impl.md:4062-4065 specifically preserves
synchronous neighboring-row re-recognition, although neither is needed to invent a stronger global provider timing
promise. Whole pinned-range history for terminal-files.js/test.js shows only 04d85f0c introducing this test and
f560058da19e02716e6f943883a9508b447aff4e deleting it with xterm. This is a historical intermediate-commi

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g135/l3`.

### g135/l3

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 14 (finding, open):

Reviewed commit: 04d85f0c98a9b635709d00b056719a0baa7fd3fc. TLDR: the new test says it protects immediate file-link
recognition, but would still pass if recognition delivered the same result asynchronously; it cannot catch the
scheduling regression it explicitly claims to prevent. Source: test comment at 398-399 says 'Recognition must stay
synchronous for ordinary supervisor files. Retiring

- checkout confinement must not add host Stat requests before offering a path.' The test at 400-413 installs a request
  stub that throws, then line 410 awaits a Promise resolved by provideLinks and only compares the eventual paths at 411.
  No assertion checks whether the callback ran before provideLinks returned. The host-lookup stub does protect the
  no-request half, but both synchronous and deferred no-request callbacks satisfy the timing-blind observable.
  Production provideLinks at terminal-files.js:672-677 currently calls back synchronously, so no production timing
  regression is alleged; production correctness does not establish the test's stated timing oracle. The review charter's
  test checklist requires an observable that distinguishes competing mechanisms, and the test itself declares the
  synchronous contract. Related SPEC.md:3126 requires immediate underline/checking feedback; SPEC_impl.md:4062-4065
  specifically preserves synchronous neighboring-row re-recognition, although neither is needed to invent a stronger
  global provider timing promise. Whole pinned-range history for terminal-files.js/test.js shows only 04d85f0c
  introducing this test and f560058da19e02716e6f943883a9508b447aff4e deleting it with xterm. This is a historical
  intermediate-commit test-oracle defect within the assigned patch, absent from the final pinned tip; it is not a
  surviving product bug. Suggested bucket: other. Could-be-highest: false; no established security or loss-of-work route
  from this test gap. confidence: confirmed by source/JavaScript Promise semantics, no execution or reproduction.
  proposed_drop: false; later deletion is a material disposition caveat for the parent, not a silently accepted drop.
  possible_cover: none; interface context has no same timing-oracle finding, and no prohibited coverage sources
  searched. Fix direction at the reviewed revision: capture callback delivery in a local value and assert it immediately
  after provideLinks returns, retaining the no-request assertion; alternatively narrow the comment/title if synchronous
  delivery is not the intended protected contract.

Returned proposal (claims-085, event 14): **retain-verified**, suggested bucket **other**.

The intermediate test explicitly claims synchronous recognition but only checks eventual callback output. Its later
deletion is material disposition context, not a source refutation of the historical test defect.

Original stronger consequence: A test claiming immediate recognition would miss an asynchronous no-request callback
regression; other/test accuracy only, never a security or loss-of-work claim.

Unresolved premise or limitation: None for the historical test-oracle gap. It no longer exists at the final tip, and no
surviving product timing failure is claimed.

Proposed basis: Retain historical in-range test defect with explicit later removal. Parent may decide how to record an
already-deleted test; do not present it as currently actionable.

Existing parent disposition (event 14): rejected. Parent read full claims085.04d85f0c newly adds test398-413 claiming
synchronous callback recognition but awaits resolve-only promise before assertions; throwing request stub proves no
hostrequest, not callback-before-return. Actual production callback is synchronous; true historical missingtiming
oracle, not runtimefailure. f560058 deletes this retired XtermFileLinksAddon test and adapter; absent at00de. Preserve
defect and later removal without residual actionable queueitem. This does not refute required retained grid recognition
tests or assertion fidelity generally; no highest consequence.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g135 record](records/review-through-00de0b6c/groups/g135.json). Record ID: `l3`.
