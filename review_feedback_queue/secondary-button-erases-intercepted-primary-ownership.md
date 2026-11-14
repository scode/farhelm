# A second mouse button erases an intercepted primary link press's ownership

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

Holding a web-link click and pressing a second mouse button loses the primary click's interception, so its subsequent
drag/release reaches the terminal program and its original link no longer opens.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed by source and
standard mouse-event semantics. W3C Pointer Events 4 dated 2026-10-08, section 3.1.1.1
(https://www.w3.org/TR/2026/WD-pointerevents4-20261008/#chorded-button-interactions) states that the Mouse Event model
produces mousedown and mouseup per button; this code uses those events, not non-overlapping pointerdown/pointerup. No
runtime measurement or native default-menu behavior is claimed or needed for the shown handler route. Fix: independent
primary ownership through its matching release while preserving secondary reports.

Source locations: 7509285e647ea86ccd2cee7905675dc87dad9b6f:crates/farhelm-ui/assets/grid-terminal.js:1657-1718.

In the reviewed commit, a primary web-link press under tracking/default interception stores withheld=true at 1684-1698.
Press a second button before releasing the primary: link is null for a non-primary button, withheld is false, and
startGesture(state) at 1718 stops/replaces the existing gesture (1657-1661). Subsequent moves prioritize the held
primary button (grid-input.js:489-492 at a4f30ed8574d5e50f01771b90b1020b959f6acd4) but read withheld=false (1575-1576),
and its release is sent (1588). The original link is also lost. Preserve primary ownership independently through its
matching release.

The one gesture stores withheld at 1688-1698. Every reporting press calls startGesture(state) at 1718; startGesture
stops and replaces an existing gesture at 1657-1661. A secondary press is never withheld (link requires ev.button === 0
at 1684), so it replaces the held primary's withheld=true with false. Subsequent moves and primary release use only that
replacement's withheld at 1575-1576 and 1588. SPEC_impl.md:1657-1666 explicitly requires independent primary ownership
through release, including another button pressed during interception.

Holding a web-link click and pressing a second mouse button loses the primary click's interception, so its subsequent
drag/release reaches the terminal program and its original link no longer opens. Exact trigger: mouse-reporting program,
default intercept setting, hovered web link, primary press followed by another button press in the terminal before
primary release. Source: primary link press gets withheld=true, but non-primary link is null (tip:1690-1704), and every
reporting press replaces the singleton gesture (1663-1667,1724). Moves/release then use withheld=false (1581-1594);
grid-input.js:489-493 prioritizes the held primary bit. Caller terminal.js:5342-5348 supplies only interceptLinkPress,
and terminal-links.js:195-197 accepts only primary web presses; no second independent guard exists for the grid.
Supervisor grid_input.rs:672-729 encodes the received primary motion/release without requiring that it previously
encoded a down, so missing original down does not suppress the leak (choose ordinary Drag/Motion reporting with SGR).

Possible coverage and caveats: no exact coverage. Tip SPEC.md:2304-2331 requires withheld drag/release;
SPEC_impl.md:1645-1669 explicitly covers overlapping buttons. SPEC.md:3966-3969 accepts some pre-redraw actions but
preserves named guards and does not authorize losing press ownership through a held overlap. Planned bucket is empty
(TODO.md:23-25); targeted searches of plans/queue, BUGS.md, INDEX and TRIAGE_OUTCOMES found no interception match.
Existing queue secondary-press-suppresses-primary-selection-copy.md:1-31 and ledger TRIAGE_OUTCOMES.md:19387-19396
concern terminal.js clipboard copying, not forwarded grid input or lost link activation. Filters lack this exact
trigger/consequence.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g161/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 7509285e647ea86ccd2cee7905675dc87dad9b6f. Tip
persistence rechecked at ca9b0a19856dc136f63ad2d14f1076027edce1eb: grid-terminal.js:1581-1594,1663-1724.. This was a
source-only review; runtime occurrence and frequency were not measured.

## Abbreviated review through 00de0b6c

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g010/f4`, `g082/l1`.

### g010/f4

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 12 (finding, open):

Reviewed commit: f560058da19e02716e6f943883a9508b447aff4e. Discovery pass 2: f4 lead resolved to a source-confirmed
replacement-documentation defect; the supporting grid behavior predates the range and is not attributed as a newly added
handler. TLDR: The new grid-only explanation promises that an intercepted primary link gesture never reaches the
terminal program, but pressing another mouse button before releasing the primary loses that ownership. The program can
receive a primary release or drag for the press Farhelm withheld, and the intended link click is lost. Details:
SPEC_impl.md:1778-1784 newly states that the grid reports neither the press nor its drag nor its release to the program.
Under mouse reporting and default intercept, grid-terminal.js:1773-1792 withholds the primary link press and starts a
reporting gesture carrying withheld=true. A middle-button mousedown in the same grid has pressed/link=null (1773-1774),
hence withheld=false, and startGesture at 1747-1751 unconditionally stops/replaces the earlier gesture. Release the
middle button while the primary remains held: onDocumentMouseUp:1666-1674 reports that middle release but keeps the
replacement gesture because ev.buttons is nonzero. A later primary-held move passes 1657-1658 without suppression; the
primary release passes 1670 because ending.withheld is now false. That release also has ending.link=null, so 1689-1693
does not activate the originally pressed link. grid-input.js:508-517 recognizes these buttons and maps primary-held
motion to button 0; 555-559 admits releases and dragged motion, and 575-584 deduplicates only repeated moves. The
supervisor does not restore ownership: grid_input.rs:688-745 encodes valid primary Up and held Move directly for
ordinary SGR click/drag modes. No reproduction or runtime incidence is claimed; this is the conditional source path
through the shipped event handlers. Contract: SPEC.md:3084-3088 explicitly requires a default intercepted primary web
press to be withheld together with its release and any drag starting there, while other buttons keep their behavior. The
new root implementation description falsely presents that as a maintained grid invariant; per-button ownership must
survive a second press or the documented guarantee must be explicitly reconciled with the product contract. Full-range
attribution: exclusive base 6e423e30068b62c6cfa276c6d720ee35bb70387a grid-terminal.js already has the same
gesture.withheld/ending.withheld and replacing startGesture path at 1677,1690,1768,1799. Thus no newly introduced
runtime gesture bug is alleged for existing grid terminals. The assigned patch instead replaces an xterm-specific
explanation (parent SPEC_impl.md:1822-1839) that explicitly preserves independent primary ownership even when another
button installs reporters, with the newly unconditional grid claim at 1778-1784. The parent raw path's separate
withheldPrimary state is at terminal.js:4189,4552-4611; its full capture/cleanup logic remains to be reread to qualify
the removed-path comparison, but is not needed to establish the new claim's contradiction with the checked grid
handlers. Pinned tip SPEC_impl.md:1779-1785 retains the new explanation and tip grid-terminal.js retains the same
suppression/replacement predicates; no endpoint repair shown by these direct checks. Suggested bucket: other, inaccurate
implementation contract and rare mouse-input interference; no established security or loss-of-work route.
Could-be-highest: false on this evidence; no destructive or privileged program action is inferred from a mouse release.
Confidence: confirmed conditional handler/classifier/encoder and documentation contradiction, not execution-observed.
proposed_drop: false. possible_cover: none among the complete refreshed supplied interface records; no prohibited
coverage searches. Material caveats: an ordinary single-button intercepted click behaves correctly; the trigger needs
another recognized button press inside the grid before primary release, with program mouse reporting on. The original
suspected unmatched primary release/drag consequence is preserved; source adds that the single gesture also loses its
original link identity. The remaining raw-path comparison is supplemental attribution, not an unresolved premise of the
new documentation finding. No runtime test, build or reproduction. Supplemental comparison completed: parent
terminal.js:4186-4195 stores withheldPrimary independently and filters onData through suppressMouseReport; :4576-4584
captures withheld primary motion/release and only clears that state on a primary down/up, so a middle press/release does
not erase its primary ownership. :4538-4611 was read in full for the relevant screen listener, capture and teardown.
This settles the raw-path comparison without asserting an unrelated retired-code defect. Base SPEC.md:3064-3071 already
contains the same product withholding promise, so no new product requirement or new grid runtime regression is
attributed to this range; the assigned new grid-specific implementation explanation is the independently incorrect
statement. Direct endpoint grid-terminal.js:1640-1690 and 1735-1810 confirms unchanged replacing gesture and suppression
conditions, and endpoint SPEC_impl.md:1779-1785 retains the false unconditional description. Correct exact reviewed root
assertion is SPEC_impl.md:1781, inside the paragraph starting 1778 (location remains 1779 for the replacement
mechanism). All readily checkable attribution, helper, classifier, encoder and persistence premises are resolved; no
open lead remains. Existing runtime implementation, documentary error and removed-path comparison are kept separate. No
accepted parent lowering/drop or runtime observation added.

Returned proposal (claims-005, event 12): **retain-verified**, suggested bucket **other**.

The runtime trigger and consequence exactly match an existing item. The in-range replacement specification adds a useful
documentation extension: it newly describes the grid as preserving an invariant that its existing singleton gesture does
not preserve. Extend the existing item with this new assertion rather than create another runtime finding.

Original stronger consequence: The original lead alleged an unmatched primary release or drag. The final claim preserves
that consequence and adds lost link identity. No destructive or privileged action from the mouse report was established.
Ordinary single-button intercepted clicks remain outside the trigger.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Extend the existing item with the false in-range grid-specific promise and its attribution. Runtime
scope is exact existing coverage; the documentation evidence is an extension, not grounds to lose the claim.

Existing parent disposition (event 12): verified. Parent explicitly accepted named retain-verified proposal from
claims-005 after inspecting it. The runtime trigger and consequence exactly match an existing item. The in-range
replacement specification adds a useful documentation extension: it newly describes the grid as preserving an invariant
that its existing singleton gesture does not preserve. Extend the existing item with this new assertion rather than
create another runtime finding. Extend the existing item with the false in-range grid-specific promise and its
attribution. Runtime scope is exact existing coverage; the documentation evidence is an extension, not grounds to lose
the claim. Checked source/results: ["f560058da19e02716e6f943883a9508b447aff4e:SPEC_impl.md:1778-1784, especially 1781,
newly promises no forwarded press, drag or release for an intercepted link press.","At f560058,
grid-terminal.js:1747-1758 replaces the singleton gesture. 1773-1792 gives a second non-primary reporting press
link=null and withheld=false. 1651-1674 subsequently checks only that replacement's withheld flag; 1689-1693 also loses
the original link. This establishes the conditional handler route, not observed runtime
incidence.","6e423e30068b62c6cfa276c6d720ee35bb70387a:grid-terminal.js:1677,1690,1768-1769,1799 already has the same
suppression and replacement predicates. The runtime defect predates this range; the newly written grid-specific
explanation is the in-range addition.","At 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db,
grid-terminal.js:1652-1669,1742-1746,1768-1802 and SPEC_impl.md:1779-1785 retain the mechanism and assertion."] Current
coverage/results: ["SPEC.md:3077-3085 requires primary press/release/drag withholding and preserves other buttons;
SPEC_impl.md:1779-1785 incorrectly asserts that behavior rather than accepting the overlap
exception.","INDEX.md:1267-1269 points to secondary-button-erases-intercepted-primary-ownership.md. Read that complete
file: exact runtime trigger, forwarded primary motion/release, lost activation, and proposed independent ownership all
match. Its historical SPEC_impl reference predates the new grid-only paragraph, so the new inaccurate assertion is
useful additional evidence.","TODO.md Planned is empty. Targeted plans/queue searches found no unlanded plan fixing this
ownership state; the tmux series is landed per plans/REPORTS.md:29-32. BUGS.md has no match. Active TRIAGE_OUTCOMES.md
has no matching interception/secondary-button decision.","FILTER.md does not establish that a deliberate
overlapping-button gesture meets its rare-trigger boundary. No filter proposed."] Exact cited cover: ["Existing queue
TLDR: Holding a web-link click and pressing a second mouse button loses the primary click's interception, so its
subsequent drag/release reaches the terminal program and its original link no longer opens."] Original stronger
consequence and missing premise remain preserved: "The original lead alleged an unmatched primary release or drag. The
final claim preserves that consequence and adds lost link identity. No destructive or privileged action from the mouse
report was established. Ordinary single-button intercepted clicks remain outside the trigger."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g010 record](records/review-through-00de0b6c/groups/g010.json). Record ID: `f4`.

### g082/l1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 20 (candidate, open):

Reviewed commit: f560058da19e02716e6f943883a9508b447aff4e. Original consequence preserved: a second button pressed
during a withheld primary web-link gesture replaces the grid's ownership state, allowing its primary release or drag to
reach the program without the corresponding primary press. Original source basis preserved: grid-terminal.js:1773–1809
creates/replaces the gesture and move/up handlers at 1649–1670 consult only the new withheld flag; grid-input.js:555–560
and grid_input.rs:688–749 admit and encode the resulting release. SPEC.md, Terminal experience, requires withholding the
primary press with its release and drag. This turn checks whole-range attribution rather than relying only on the
immediate parent: exclusive base 6e423e30068b62c6cfa276c6d720ee35bb70387a already has the same gesture.withheld move
filter at grid-terminal.js:1677, ending.withheld release filter at :1690, startGesture replacement at :1768, fresh
withheld computation at :1799, and unconditional startGesture(state) at :1829. Immediate parent
1ed482dde220452e424973faf38a5e1318986333 has those same locations/logic. Full pinned-range file history shows
f560058da19e02716e6f943883a9508b447aff4e and later 61e632323dbc6b4a35ed27bf5b9833102ffd5a4e; earlier diff review of the
selected commit established only comment changes in these gesture blocks. Thus parent inheritance is supplemented by
direct base evidence, not treated alone as proof of predating the range. Suggested bucket: other on established
partial-input consequence; application-specific serious harm is unestablished. Confidence: source-confirmed mechanism
and pre-range presence; no runtime observation. proposed_drop: retained as a pre-range issue outside this assigned
deletion's introduced behavior, pending parent scope/coverage judgment; not accepted here. possible_cover: caller
supplied matching pre-existing grid ownership claim; exact queue identity was not searched. Could-be-highest: false on
established premises. Caveats: old xterm had its own surviving-primary flag but never used it for host-grid sessions. No
test or documentation defect is dismissed by this production inheritance; none is newly claimed here. No open source
lead remains.

Returned proposal (claims-053, event 20): **out-of-range**, suggested bucket **other**.

The partial-gesture leak is real in source but already exists at the exclusive base, and an existing queue item records
the exact trigger and consequence. No useful new extension is required.

Original stronger consequence: Initial lead asked whether a second button replaces withheld-primary state; later source
checks established leaked release/drag. Immediate-parent inheritance was initially the only scope argument, then latest
event supplied direct exclusive-base evidence. The real consequence is preserved, not refuted as correct behavior.

Unresolved premise or limitation: No application-specific severe harm or loss-of-work route established. No
runtime/native mouse sequence observed.

Proposed basis: Outside introduced behavior in the full pinned range, with exact existing queue coverage as an
additional basis. Do not infer that grid multi-button ownership is correct or dismiss separate tests/docs defects.

Existing parent disposition (event 20): duplicate. Parent corrects its consolidation conflict after reading complete
claims053. This record's runtime chorded-gesture leak exists at exclusive base and exactly matches the existing queue
item read in full; it supplies no additional runtime caller. But g010/f4 under this same root retains the newly false
in-range withholding explanation as useful extension evidence. Join that verified group as a duplicate, preserving this
older-runtime attribution and not treating production correctness or existing runtime coverage as a refutation of the
new documentation defect. No highest consequence asserted.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g082 record](records/review-through-00de0b6c/groups/g082.json). Record ID: `l1`.

## Abbreviated review through 00de0b6c

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g174/l2`.

### g174/l2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 13 (candidate, open):

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513. TLDR: a secondary-button press can replace a grid
withheld-primary gesture, after which primary release/motion can be reported to the program; this is a pre-range source
concern, not a regression introduced by the assigned test removal. Original consequence preserved: unexpected program
mouse release or primary motion contrary to the withheld-gesture contract, with program-specific further actions
unverified. Source checks from initial pass: grid-terminal.js:1782-1831 builds the secondary nonwithheld state and
startGesture:1768-1780 replaces the earlier state; onDocumentMouseUp:1686-1694 uses that replacement.
mouseMessage:1664-1668 calls modeReports:556-560, which accepts releases when reporting is on, and
createMouseDeduper:576-585, which only deduplicates moves. New synthesis check: git diff from the exclusive range base
6e423e30068b62c6cfa276c6d720ee35bb70387a to647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513 for grid-terminal.js and
grid-input.js is empty, directly establishing this is pre-range, rather than inferring that merely from the prior log
hash befe5c72f7af72bb7e2fae339c0f4ca52ff816da. Caller independently confirms it is a known pre-range secondary-button
issue. Parent suite-terminals.ts:80-88 already ran deleted overlapping-button cases only against xterm, so they never
tested this grid mechanism. Suggested bucket: other on the established frontend-message consequence alone; no
established security or loss-of-work route. Confidence: confirmed source attribution; no runtime behavior observed.
proposed_drop: pre-range implementation and no causal change in assigned scope, so do not report as a new range finding;
retain this record for the caller's independent recording/drop check. This does not assert that loss of useful
old-renderer tests is harmless; that test-only question is separately reassessed under c4. Could-be-highest: no
demonstrated shipped security or loss-of-work consequence; a program-specific destructive response has not been
established. possible_cover: caller says the pre-range mechanism is known; no queued findings or forbidden coverage
metadata were searched. No tests or reproductions run. No unresolved premise remains for this attribution decision.

Returned proposal (claims-109, event 13): **out-of-range**, suggested bucket **other**.

The shown gesture replacement mechanism exists unchanged at the exclusive base, and the existing queue item records
exactly that trigger and consequence. It is not introduced by deleting tests that already ran only against xterm.

Original stronger consequence: Line 6 was an open interface lead; line 7 attributed it outside the assigned patch; line
13 strengthens attribution to pre-range with an empty full-base diff. The unexpected program release/motion consequence
remains preserved rather than refuted. Related c4 independently concerns loss of old-renderer tests.

Unresolved premise or limitation: No program-specific security or user-work-loss response to the unexpected report was
established; source attribution has no remaining premise.

Proposed basis: Exclude as a new in-range finding and retain its alias to the already queued product defect. The
existing item is exact coverage if the parent chooses duplicate disposition instead; do not claim the behavior is
correct or infer an unshown destructive program response.

Existing parent disposition (event 13): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g174 record](records/review-through-00de0b6c/groups/g174.json). Record ID: `l2`.
