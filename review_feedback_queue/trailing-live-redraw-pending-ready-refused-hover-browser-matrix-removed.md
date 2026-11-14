# Preserve browser redraw coverage for pending and refused file hovers

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: removing the four live redraw cases loses useful browser regression checks that an unchanged file
hover keeps one pending lookup and a stable ready/refused disclosure while real output redraws underneath it. The
initial candidate was too dismissive: absence of a current product failure does not refute a test-only regression.
Source: deleted terminal-files.spec.ts parent61-196 observes actual xterm onRender ticks and tooltip mutations, holds a
real stat reply through repeated output, checks pending and answered request counts, pointer readiness for plain/OSC and
file/folder outcomes, then verifies leaving and returning makes a fresh lookup. At this intermediate commit
SPEC_impl.md:2294-2311 expressly still serves adopted old terminals, and4077-4085 describes their leave/re-entry
contract; inability to create a new old terminal is a fixture-portability issue, not proof those checks were useless.
Later 1ed482dde220452e424973faf38a5e1318986333 ends the old server at startup, and
f560058da19e02716e6f943883a9508b447aff4e removes xterm from the page; that ends the old-renderer-specific coverage need,
without retroactively erasing the intermediate gap. Shared surviving promise: tip SPEC_impl.md:3596-3604 now explicitly
assigns same-task leave/re-entry and hover-evidence retention to grid refreshHover. Retained tests do provide
substantial partial coverage: deletion-revision terminal-files.test.js:486-568 (tip428-509) directly simulates
leave/rehover for pending, ready and folder states with a fake task clock; tip grid-attach.spec.ts:1062-1097 feeds
identical/different plain file ro

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g174/c3`.

### g174/c3

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 11 (finding, open):

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513; synthesis checked through tip
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db. TLDR: removing the four live redraw cases loses useful browser regression
checks that an unchanged file hover keeps one pending lookup and a stable ready/refused disclosure while real output
redraws underneath it. The initial candidate was too dismissive: absence of a current product failure does not refute a
test-only regression. Source: deleted terminal-files.spec.ts parent61-196 observes actual xterm onRender ticks and
tooltip mutations, holds a real stat reply through repeated output, checks pending and answered request counts, pointer
readiness for plain/OSC and file/folder outcomes, then verifies leaving and returning makes a fresh lookup. At this
intermediate commit SPEC_impl.md:2294-2311 expressly still serves adopted old terminals, and4077-4085 describes their
leave/re-entry contract; inability to create a new old terminal is a fixture-portability issue, not proof those checks
were useless. Later 1ed482dde220452e424973faf38a5e1318986333 ends the old server at startup, and
f560058da19e02716e6f943883a9508b447aff4e removes xterm from the page; that ends the old-renderer-specific coverage need,
without retroactively erasing the intermediate gap. Shared surviving promise: tip SPEC_impl.md:3596-3604 now explicitly
assigns same-task leave/re-entry and hover-evidence retention to grid refreshHover. Retained tests do provide
substantial partial coverage: deletion-revision terminal-files.test.js:486-568 (tip428-509) directly simulates
leave/rehover for pending, ready and folder states with a fake task clock; tip grid-attach.spec.ts:1062-1097 feeds
identical/different plain file rows after a ready answer and verifies stat counts/downloads; retained real-stack
terminal-files.spec.ts checks changed OSC text cancels evidence. These do not preserve the removed
actual-render/mutation oracle and full pending-plus-answered plain/OSC file/folder browser matrix. This finding is about
loss of that integration check, not absence of all hover tests or proof of a broken production hover. Fix direction:
carry the repeated-output/pending-and-refused cases over to grid geometry and actual grid row updates, including
mutation and request-count assertions; do not restore a retired renderer just to run its old fixture. Suggested bucket:
other, test-only. Confidence: confirmed removal and retained-test limits by source analysis. proposed_drop: false; prior
proposed-drop rationale is withdrawn. Could-be-highest: no established shipped security or loss-of-work route; a
hypothetical stale admission bug is not claimed. possible_cover: none known without forbidden searches; distinct from
l1's production overlay/CSS gap. No runtime execution or reproduction performed. Original consequence retained and
narrowed correctly: loss of useful regression protection, with old-specific need limited to the intermediate
supported-old revision and shared hover lifecycle still present at tip.

Returned proposal (claims-109, event 11): **retain-verified**, suggested bucket **other**.

The deleted live-render test held a real lookup across repeated output and counted transient tooltip mutations for
plain/OSC file/folder outcomes. The remaining ready plain-file grid case and fake-clock unit cases cover parts, not that
browser matrix.

Original stronger consequence: Line 5 proposed dropping because tests were xterm-specific, grid coverage remained, and
no functional regression was shown; line 11 withdraws that rationale. Initial record mentioned possible security
relevance of stale target admission without showing a route. Preserve that distinction, the old-support interval,
existing partial coverage and the need for grid-based replacement.

Unresolved premise or limitation: No broken shipped hover or harmful stale admission route is established. At tip only
the shared lifecycle needs replacement coverage; old-specific instrumentation is obsolete.

Proposed basis: Retain concrete browser-test loss with other priority. Do not treat partial unit/grid coverage or later
retirement as refuting the intermediate gap. Earlier security-related dismissal wording remains for the parent's drop
register.

Existing parent disposition (event 11): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g174 record](records/review-through-00de0b6c/groups/g174.json). Record ID: `c3`.
