# Old-terminal flood and paused-remount checks disappear before retirement

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: sessions still using old terminals lose their real-browser checks for lossless flood delivery,
observed pause/resume, stall handling and clean state after leaving a paused attachment. This is a historical
regression-test coverage loss, not a demonstrated current production failure.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g175/c2`.

### g175/c2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 10 (finding, open):

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513. TLDR: sessions still using old terminals lose their
real-browser checks for lossless flood delivery, observed pause/resume, stall handling and clean state after leaving a
paused attachment. This is a historical regression-test coverage loss, not a demonstrated current production failure.
Details: deleted parent terminal-flood.spec.ts:671-739 requires all 800,000 records exactly once/in order plus observed
pauseCount>=1 and matching resumeCount, independently of the retained tail. Its stalled-viewer case at :780-937 forces
an actual xterm watermark pause, requires the roughly sixty-second interval, visible stalled reason and no automatic
recovery, then reattaches. Its same-realm case :987-1092 proves the old mount still paused, restores the shared
prototype without releasing that mount, drains the producer off-screen, switches away/back without reloading the realm,
and requires no inherited pause or bare resume on the replay-only mount. At the reviewed commit SPEC_impl.md:2294-2311
says the supervisor still adopts and serves an earlier tmux server, and :2956-2978 preserves old agents/tabs and
old-terminal support. terminal.js:4712-4753,5335-5371 still implements the per-mount
pendingWrite/paused/write-completion watermark mechanism. Competing regressions these deleted assertions distinguish
include dropping an earlier flood batch while preserving the final retained tail, disabling watermark accounting while
accepting all output, or moving paused/pendingWrite state to page scope so a new attachment inherits a prior pause. No
mutation/reproduction was run. Retained-test checks: targeted pinned-tree searches across e2e, Rust end-to-end tests,
helm and supervisor source for flood verifier, gated flood, pause counters, watermark, paused-remount and reset-flow
witnesses find only the grid flood and a healthy remount hook assertion. terminal.spec.ts:535-577 asserts zeroed hook
state but its host-only fixture never enters a byte-stream watermark pause; it cannot exercise inheriting an actually
paused old mount. helm terminal.rs:1418-1463 injects pause/resume JSON into a scripted peer and verifies forwarding, not
browser counter transitions or whole live old-terminal delivery. The previously read retained grid-flood, grid-attach,
grid-stream and relay tests are useful but run different mechanisms and do not replace these old-page oracles. Suggested
bucket: other, established harm is regression coverage in maintainer tooling. Confidence: confirmed source/contract
analysis only. proposed_drop: false; prior proposals based on no functional regression, grid tests, fixture portability
or later retirement are withdrawn, while their original text remains in prior c2 events. possible_cover: supplied
g176/c2 is the distinct late-replay-write completion boundary, and g176/c3 is simultaneous sibling isolation; retain
provenance without claiming this is their same witness. Could-be-highest: no established security or loss-of-work route
from deleting a test alone; possible bugs named above are sensitivity examples, not alleged shipped failures. Later
disposition: dbfc69751215dbf15e1d31d5bed73c3754c4ebc4 retires old terminals at startup, and
f560058da19e02716e6f943883a9508b447aff4e removes the byte renderer/writeBytes. That makes the old-specific checks
obsolete later, not at 647f0227. Historical fix direction: retain these assertions through an adopted-old fixture until
retirement, or remove them together with their consumer. Do not restore a retired renderer on the pinned tip merely to
run them. Original consequence preserved: lost whole-stream, flow/stall and paused-remount regression checks. No
prohibited coverage sources searched and no runtime execution.

Returned proposal (claims-109, event 10): **retain-verified**, suggested bucket **other**.

The deletion removes browser-level whole-stream verification, observed watermark pause/resume, real stall and
actually-paused same-realm remount witnesses during the interval old terminals remain supported. Grid tests exercise
different mechanisms.

Original stronger consequence: Lines 2/4/5 proposed dropping based on different grid mechanisms, surviving tests and no
invalid surviving oracle; line 8 reopens that rationale and line 10 explicitly withdraws it. Original lost whole-stream,
flow/stall and paused-remount consequences remain. Existing grid tests are useful, not exhaustive old-path substitutes.
Neighboring g176 late-flush/sibling witnesses are distinct. Caller-supplied retirement work-loss/duplication provenance
remains separate and is not silently accepted or dropped by this record.

Unresolved premise or limitation: No shipped lossless-delivery, watermark or remount failure was reproduced or alleged.
Sensitivity examples are hypothetical regressions. At tip these old-specific mechanisms are gone; do not restore them.

Proposed basis: Retain historical maintainer-test coverage loss with other priority. Later intentional retirement does
not erase in-range test loss. No product security or user-work-loss route is part of this test-deletion claim.

Existing parent disposition (event 10): rejected. Full109:647 removes800000record whole-stream
correctness/observedwatermarkpause-resume/>55srealstallclosedoriginalsocket/no-reconnect
andactuallypausedsame-realmremountreset browserwitnesses whileoldconsumersstillserved. Differentgridpacing
orhealthyunpausedremountcannotreplacethem. Historicalprotectionlossreal, previousdifferentmechanisms/noinvalidoracle
dismissalwithdrawn. dbfc/f560retireexactoldconsumerandcallbackflowbytip; no obsoletecurrentfixturefix or
actualdeliveryfailure/worklossclaimed.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g175 record](records/review-through-00de0b6c/groups/g175.json). Record ID: `c2`.
