# Keep program-observed link ownership checks through Settings and mounted tabs

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: deleting the link-gesture suite removes useful end-to-end checks that changing the link setting
actually changes browser opens and program mouse reports in already mounted agent and shell terminals. Separate
UI/pure-policy tests do not exercise that shared wiring together. This revises the original candidate's dismissal: no
observed production failure is required to establish removal of a useful regression test. Original parent
terminal-link-clicks.spec.ts:324-423 uses Settings for all three modes, verifies the shared page choice and unchanged
terminal objects, and checks real program-acknowledged mouse counts and opens on agent plus shell tab, followed by
reload seeding. Original584-630 likewise changes the choice via Settings and checks program-only then both/intercept
without mouse reporting. Original28-113 and428-580 additionally cover drag-versus-activation, forced selection, withheld
notices and overlapping xterm document reporters. At deletion, adopted old terminals still run and are served under
SPEC_impl.md:2294-2311, so deleting their unique browser-program checks creates an intermediate regression-coverage gap
even though the removed creation switch prevents the old fixture from running unchanged. Later
1ed482dde220452e424973faf38a5e1318986333 ends old terminals and f560058da19e02716e6f943883a9508b447aff4e removes the
page's old renderer. Xterm-specific bookkeeping tests are then obsolete, but the shared preference/application contract
remains: tip SPEC_impl.md:1772-1788 says PreferencesGate publishes the choice, every mounted terminal reads it without a
remount,

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g174/c4`.

### g174/c4

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 12 (finding, open):

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513; checked remaining assertions through
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db. TLDR: deleting the link-gesture suite removes useful end-to-end checks that
changing the link setting actually changes browser opens and program mouse reports in already mounted agent and shell
terminals. Separate UI/pure-policy tests do not exercise that shared wiring together. This revises the original
candidate's dismissal: no observed production failure is required to establish removal of a useful regression test.
Original parent terminal-link-clicks.spec.ts:324-423 uses Settings for all three modes, verifies the shared page choice
and unchanged terminal objects, and checks real program-acknowledged mouse counts and opens on agent plus shell tab,
followed by reload seeding. Original584-630 likewise changes the choice via Settings and checks program-only then
both/intercept without mouse reporting. Original28-113 and428-580 additionally cover drag-versus-activation, forced
selection, withheld notices and overlapping xterm document reporters. At deletion, adopted old terminals still run and
are served under SPEC_impl.md:2294-2311, so deleting their unique browser-program checks creates an intermediate
regression-coverage gap even though the removed creation switch prevents the old fixture from running unchanged. Later
1ed482dde220452e424973faf38a5e1318986333 ends old terminals and f560058da19e02716e6f943883a9508b447aff4e removes the
page's old renderer. Xterm-specific bookkeeping tests are then obsolete, but the shared preference/application contract
remains: tip SPEC_impl.md:1772-1788 says PreferencesGate publishes the choice, every mounted terminal reads it without a
remount, and other clients reseed on reload. Retained tip grid-attach.spec.ts:684-773 exercises withheld notice and
default interception plus program-only activation, but sets farhelmTerminalLinkClickMode directly and uses a fake
socket; it does not connect Settings/API/PreferencesGate to mounted agent+tab behavior or test the both choice through
that chain. Retained terminal-links.test.js:86-95 covers the pure policy. settings.spec.ts:525-596 and1932-1989 checks
real preference writes, controls and supersession, not corresponding program mouse/open outcomes. Pinned-tree grep for
the page mode, link ownership and terminal_link_click confirms no replacement combined check in these suites. Fix
direction: port the Settings-to-agent-and-tab program-observed ownership case to grid terminals using the existing real
mouse-mode fixture and ordered acknowledgement, retaining positive beside-link and all-mode checks. This does not
propose restoring xterm-specific bookkeeping or reporting the pre-range secondary-button issue as introduced here.
Suggested bucket: other (test-only integration-coverage loss). Confidence: confirmed removal and surviving-test limits
from source; no runtime behavior observed. proposed_drop: false; prior proposed-drop rationale withdrawn.
Could-be-highest: no established shipped security or loss-of-work consequence. possible_cover: l1/g176-l2 is the
distinct production overlay/CSS portion of the same file; l2 is the separately attributed pre-range grid overlap
mechanism. Material caveats: existing component, policy and Settings tests remain useful, and the deleted old-only case
never tested the grid implementation itself. The assertion lost here guarded shared application wiring; its replacement
should guard that surviving promise on grid. No tests or reproductions run.

Returned proposal (claims-109, event 12): **retain-verified**, suggested bucket **other**.

The removed test connected all three Settings choices, real preference publication, unchanged mounted agent/shell
terminals, program-acknowledged mouse counts and browser opens. Retained Settings and component tests split that chain.

Original stronger consequence: Line 8 proposed dropping remaining xterm gesture removals because no additional
functional regression was shown and separate tests existed; line 12 withdraws that rationale and retains the specific
shared Settings/application chain. Old forced-selection/overlap bookkeeping later becomes obsolete; no claim of
exhaustive missing coverage or introduction of the pre-range l2 product defect remains.

Unresolved premise or limitation: No shipped wrong opening, destructive program response or user-work loss is shown.
Deleted old-only tests never exercised grid itself; replacement should test surviving shared wiring on grid.

Proposed basis: Retain the distinct Settings/application test gap. Its initial dismissal covered program-input ownership
tests, so preserve it for a conservative independent drop-register check if the parent drops or narrows the record.

Existing parent disposition (event 12): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g174 record](records/review-through-00de0b6c/groups/g174.json). Record ID: `c4`.
