# Rewritten failed-mount test misses the grid hosting failure

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: The rewritten test claims failed grid mounts dispose safely, but its fixture cannot expose the
actual cleanup failure. When construction fails in production, cleanup replaces the original diagnostic with an
undefined-element error. The production weakness already existed; the changed test incorrectly presents a different
getter as the real grid hosting contract.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g083/l1`, `g135/l2`, `g136/l1`.

### g083/l1

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 18 (finding, open):

Reviewed commit: f560058da19e02716e6f943883a9508b447aff4e. TLDR: The rewritten test claims failed grid mounts dispose
safely, but its fixture cannot expose the actual cleanup failure. When construction fails in production, cleanup
replaces the original diagnostic with an undefined-element error. The production weakness already existed; the changed
test incorrectly presents a different getter as the real grid hosting contract. Details: terminal-files.test.js:671–677
always has a host object and its view.element getter returns host.element, safely undefined before open. The diff
changes the fixture from XtermFileLinksAddon to FileLinks, adds the claim at 614–619 that it is hosted the way the grid
hosts it, and rewrites the failed-mount test at 835–844 to claim the grid preconstruction boundary. Production
terminal.js:940–950 instead returns getGrid().element with an unguarded dereference. createGridSurface at 4302–4313
passes getGrid:() => created before assigning created. If create throws, catch4356–4358 calls gridLinks.dispose.
createGridLinks disposal reaches FileLinks.dispose641–645 -> clearHover460–468; view?.element?.querySelector cannot
guard a throwing element getter. It masks the original construction exception, preventing normal return from that link
cleanup path. A unit assertion against the always-existing fixture host cannot establish successful production rollback.
History/scope checks: parent terminal.js:1194–1204 has the identical unsafe view getter; parent5962–5973 and6015–6017
has the same preconstruction closure and dispose-before-rethrow. Parent terminal-files.js:462–469 and643–647 has the
same clearHover/dispose access. This is not a newly introduced production failure from byte-renderer removal. The
in-range finding is the rewritten test fixture and its new claim of production coverage. No independent retired-code
defect was sought. Contract: brief Test authoring review: "Assert the fixture premise before the action or measurement
that depends on it." and "Assert an observable that distinguishes the competing mechanisms." Production
terminal.js:4288–4292 promises failed component creation rolls back through mount and its links are disposed even if
never created. The test must represent that actual construction boundary or state its narrower contract. Suggested
bucket: other (misleading regression coverage and diagnostic masking). Confidence: confirmed by child/parent source and
selected test diff; no execution or reproduction. proposed_drop:false. possible_cover: caller-supplied g136/l1 is the
same interface finding; consolidate with it without accepting a drop. Material caveats: construction failure is the
trigger; no established security, process-loss or user-owned-work loss consequence. c3 only checks successfully
installed notice-driver cleanup and does not certify this path. The source does not establish that all allocations from
a throwing constructor are reclaimed; that broader leak consequence is not claimed. Whole-range attribution check in
discovery: exclusive base6e423e30068b62c6cfa276c6d720ee35bb70387a terminal.js1194–1214 already has view.element ->
getGrid().element,5990–6000 passes the unassigned created closure, and6042–6044 disposes links before rethrow. The
earlier production diagnostic-masking path therefore predates the full pinned range, not merely f560058’s parent. The
in-range rewritten test/fixture documentation remains the finding and is not refuted by that history. This matches the
supplied g135/l2 and g136/l1, not an additional discovery. No consequence or proposed-drop basis removed.

Returned proposal (claims-054, event 18): **retain-verified**, suggested bucket **other**.

The rewritten test claims grid-hosting coverage but its getter safely returns undefined where production dereferences an
unconstructed component. The production masking route predates the full range; the in-range defect is the rewritten
fixture and coverage claim. Neither fact refutes the other.

Original stronger consequence: Initial production suspicion: cleanup replaces the constructor error with an
undefined-element error. Source checks confirm that conditional path but show it predates the exclusive base. The latest
retained consequence is misleading rewritten regression coverage, with the older diagnostic failure as its concrete
counterexample. No security/process/user-work loss was claimed.

Unresolved premise or limitation: No missing premise for the fixture-versus-production source mismatch. Real
construction-failure frequency, runtime manifestation and a broader resource leak were not established.

Proposed basis: Record the in-range test fixture and explanatory-contract defect, with whole-range production
attribution and no runtime claim. Preserve the supplied g135/l2 and g136/l1 consolidation leads for the parent; no
independent source defect search was performed.

Existing parent disposition (event 18): verified. Parent explicitly accepted named retain-verified proposal from
claims-054 after inspecting it. The rewritten test claims grid-hosting coverage but its getter safely returns undefined
where production dereferences an unconstructed component. The production masking route predates the full range; the
in-range defect is the rewritten fixture and coverage claim. Neither fact refutes the other. Record the in-range test
fixture and explanatory-contract defect, with whole-range production attribution and no runtime claim. Preserve the
supplied g135/l2 and g136/l1 consolidation leads for the parent; no independent source defect search was performed.
Checked source/results: ["f560058:crates/farhelm-ui/js-tests/terminal-files.test.js:615-619 claims grid hosting; 671-678
always creates host and returns host.element; 834-849 claims failed mounts dispose safely and tests the safer getter.
The f560058 patch changes XtermFileLinksAddon to FileLinks and rewrites these claims.","f560058:terminal.js:940-950
instead reads getGrid().element. createGridSurface:4302-4313 supplies () => created before the constructor assigns
created; catch:4356-4358 disposes links before rethrowing. terminal-files.js:460-469,641-647 accesses the getter during
clearHover; optional chaining cannot suppress an exception inside that getter.","Exclusive base
6e423e30068b62c6cfa276c6d720ee35bb70387a:terminal.js:1194-1214,5989-6000,6042-6044 already has the unsafe getter,
unassigned closure and dispose-before-rethrow. The production diagnostic-masking path is outside the full range. No
broader allocation leak or work-loss consequence is established.","Source only. Construction throwing is the conditional
trigger; no runtime reproduction was attempted. Histories progress from an unverified lead through production
attribution to the narrower test defect, without withdrawing that test defect."] Current coverage/results:
["SPEC.md/SPEC_impl.md searched for failed mount, grid construction, links and rollback; The grid client describes
hosting but accepts no fixture mismatch or masking diagnostic. terminal.js:4288-4292 explicitly promises preconstruction
rollback and link disposal.","TODO.md Planned empty. Matching tmux plans rewrite retired-renderer tests, but all four
landed per plans/REPORTS.md:29-32; no unlanded plan found whose stated work fixes this getter mismatch.","BUGS.md has no
matching root. INDEX.md read in full; targeted queue and active TRIAGE_OUTCOMES.md searches for failed mount, getGrid,
FileLinks, constructor and grid-hosting terms found no exact existing item or decision. The supplied histories name
g135/l2 and g136/l1; their records are outside this assigned batch and were not inspected, so exact alias status is left
to the parent.","No FILTER.md boundary covers the complete test-oracle/coverage-description defect. The rare diagnostic
filter cannot dismiss the test defect merely because the older production route masks an error."] Exact cited cover: []
Original stronger consequence and missing premise remain preserved: "Initial production suspicion: cleanup replaces the
constructor error with an undefined-element error. Source checks confirm that conditional path but show it predates the
exclusive base. The latest retained consequence is misleading rewritten regression coverage, with the older diagnostic
failure as its concrete counterexample. No security/process/user-work loss was claimed."; "No missing premise for the
fixture-versus-production source mismatch. Real construction-failure frequency, runtime manifestation and a broader
resource leak were not established.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g083 record](records/review-through-00de0b6c/groups/g083.json). Record ID: `l1`.

### g135/l2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 9 (finding, open):

Reviewed commit: f560058da19e02716e6f943883a9508b447aff4e. TLDR: the revised failed-mount test passes a safer element
accessor than production, so it cannot substantiate its claim that startup cleanup preserves the original terminal
creation failure. Source-confirmed mismatch: noticeFixture at 671-677 always creates a host object, and view.element
returns host.element safely even with opened=false; test at 834-848 describes links created before their component and
calls dispose. Production terminal.js:945-948 instead evaluates getGrid().element, with getGrid bound to created at
4302-4311; assignment at 4313 occurs only if create returns. If create throws, catch at 4356-4358 calls
gridLinks.dispose -> FileLinks.dispose at terminal-files.js:641-645 -> clearHover at 467; view.element throws before
optional chaining can handle the absent element, so throw err is not reached. Local baseline check confirms this
production accessor and cleanup structure predate the review range: 6e423e30068b62c6cfa276c6d720ee35bb70387a
terminal.js:1194-1215 and 5981-6044 already contain getGrid().element, the unassigned created closure and the catch
calling gridLinks.dispose before throw err. Thus this is an in-range misleading test/fixture contract, not a claim that
f560058d introduced the production bug. Original suspected consequence remains loss of useful startup diagnosis when
creation fails; this event attributes only the test-premise gap to this commit. This is distinct from c4's normal-mount
adapter compatibility and is not covered/refuted by it. Suggested bucket: other for the changed test-premise gap.
Could-be-highest: no established security/loss-of-work route for this rollback finding; c3/c4's potentially highest
hypotheses remain separately recorded. confidence: confirmed source/contract mismatch and conditional exception masking,
not execution-observed; no missing premise needed for the test gap. proposed_drop: false. possible_cover: g136/l1 as
supplied by parent; parent owns deduplication, no coverage sources searched. Exact changed test contract: 'The grid's
links are created before the component whose element they

- read. Failed mounts must also dispose safely; notice listeners belong to a
- later, existing terminal element.' Fix/test direction: exercise the production accessor with component absent, or an
  equivalent throwing accessor, and ensure startup cleanup tolerates the absent component while preserving the original
  error. No runtime test or reproduction performed.

Returned proposal (claims-085, event 9): **duplicate**, suggested bucket **other**.

Same changed fixture, failed-mount assertion and production accessor as g136/l1. Consolidate into that retained finding
without losing attribution or caveats.

Original stronger consequence: Useful terminal startup diagnosis may be masked; early notes considered high startup
impact depending on provenance/trigger. Later source checks establish other for the in-range test gap, with no highest
route alleged.

Unresolved premise or limitation: None for the test mismatch and conditional masking. Additional leak, retry failure,
permanent inability to mount, security or lost user work is not established.

Proposed basis: Exact same-root consolidation with g136/l1, retaining the source-confirmed test defect and pre-range
production caveat.

Existing parent disposition (event 9): duplicate. Parent read full claims085 and canonical retained g083/l1claims054.
Same rewritten f560058 terminal-files.test615-678/834-848 uses safely existing host getter where production
createGridSurface's preassignment getGrid().element throws during failed-constructor disposal. In-range
fixture/explanation mismatch persists; production masking structure predates exclusivebase. Join one retained
test-fidelity finding with exact g083/g135/g136 provenance and all constructorfrequency/no broaderleak/nohighest
caveats. No currentfixturefailure observed, no blanket productionrollback certificate.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g135 record](records/review-through-00de0b6c/groups/g135.json). Record ID: `l2`.

### g136/l1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 8 (finding, open):

Reviewed commit: f560058da19e02716e6f943883a9508b447aff4e. Synthesis checked through pinned tip
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db. TLDR: When a terminal component fails to start, cleanup can replace the useful
startup error with an undefined-component TypeError. The rewritten failed-mount test claims to cover that lifecycle but
supplies a different view, so it cannot catch the failure. The test change introduces ineffective lifecycle coverage; it
does not introduce a new product exception. Details: the assigned change rewrites terminal-files.test.js:615-678 to
claim 'hosted the way the grid hosts it: a view whose element may not exist yet' and changes 834-848 to say 'Failed
mounts must also dispose safely'. The fixture getter at 674 returns host.element; host exists at 671 even when
opened=false, so failedMount safely yields undefined. The production getter in assets/terminal.js:945-950 instead
evaluates getGrid().element. createGridSurface at 4302-4313 passes getGrid: () => created before assigning created from
farhelmGridTerminal.create; if creation throws, catch at 4356-4358 invokes gridLinks.dispose while created is undefined.
FileLinks.dispose at assets/terminal-files.js:641-645 calls clearHover, whose line 467 evaluates this.view?.element and
throws from the getter before the catch can rethrow the original error. Optional chaining guards the view, not
exceptions inside its accessor. The outer mount catch at terminal.js:4698-4743 still handles rollback and displays
Failed to start terminal using the replacement TypeError. Established impact is ineffective test coverage and loss of
the original startup diagnostic; no additional resource leak, failed retry, permanent inability to mount, security
impact or loss of user work is established. Contract: createGridSurface's documentation at terminal.js:4288-4292
promises failed-build rollback and disposal of links even when the component never exists; the revised test's own
834-836 comment specifically asserts that failed-mount lifecycle. The test needs to exercise the actual deferred
component accessor or createGridLinks failure path, alongside making the production accessor safe for an absent
component. Source checks: entire assigned patch and resulting test, both fixture versions, FileLinks teardown, real grid
view and construction catch, and outer error handling. Parent f560058da19e02716e6f943883a9508b447aff4e^ already has the
unsafe getter at terminal.js:1199-1204 and construction catch at 5971-6017. git log over
f560058da19e02716e6f943883a9508b447aff4e..00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db for terminal.js, terminal-files.js
and terminal-files.test.js returned no commits; pinned-tip source directly retains the getter, catch, outer banner and
fixture at the same lines. No later repair to these paths exists through that tip. Suggested bucket: other (test
fidelity and masked diagnostics; the original exception already prevents startup). Confidence: confirmed by source; no
runtime execution or reproduction. proposed_drop: false. possible_cover: none; prohibited coverage sources not
consulted. Could-be-highest: false on established source consequences. Interface ownership: g135 owns FileLinks/helper
deletions; g079/g083 own actual terminal surface phases. They should receive the getter/rollback input; this group does
not claim full source coverage of their phases. Further synthesis source checks: terminal.js:3275-3276 initializes the
outer grid and ws to null; 3389-3390 assigns grid only after createGridSurface returns, and 3424 creates WebSocket
later. Thus, on the specific constructor-throw route, the outer catch has no returned component or new socket to
dispose. FileLinks constructor at terminal-files.js:298-324 stores state without registering DOM listeners or starting a
request; the helper bug itself has no established outstanding transfer/listener leak at this boundary.
grid-terminal.js:310-389 creates the model and DOM before returning its public component (end at 2142); its normal
onDispose callback at 2138 is not a constructor-failure finally. These checks support the narrow masked-diagnostic
consequence, not a new resource-loss claim. Test repair should exercise the actual production view/provider boundary or
a shared extracted view factory; copying a separately safe deferred getter into the fixture would retain the same
source-divergence blind spot. The remaining c1-c3 interaction checks produced no changes: recognition and allowlist
tests remain, live ASCII repaint rows still reach admission, and cell fixtures retain wrap/width and suffix-growth
checks. Discovery-pass attribution check now includes the full range baseline, not only the assigned commit parent: at
exclusive base 6e423e30068b62c6cfa276c6d720ee35bb70387a, terminal.js:1194-1215 already defines getGrid().element, and
5981-6044 already closes over unassigned created and calls gridLinks.dispose before rethrow in the creation catch. Thus
the production weakness predates the whole pinned review range; the repurposed fixture/failed-mount coverage promise is
the assigned in-range defect. Caller-provided discovery-neighbours.json independently records the same interface under
g135/l2 and g083/l1; preserve l1 locally and consolidate only at the parent, with no drop or lowered consequence. Other
supplied interface findings concern terminal transport/encoding, not a novel premise in this assigned test patch, and
are not expanded into this group.

Returned proposal (claims-085, event 8): **retain-verified**, suggested bucket **other**.

The changed test claims production failed-mount coverage while substituting an accessor that succeeds in the exact
absent-component state where production throws. This test defect is in-range and persists at the pinned tip.

Original stronger consequence: Cleanup can replace the useful startup exception with an undefined-component TypeError
while the repurposed failed-mount test passes. The retained claim is ineffective lifecycle coverage plus conditional
masked diagnosis, other; no highest consequence was alleged.

Unresolved premise or limitation: None for the fixture mismatch or conditional diagnostic replacement. No additional
resource leak, failed retry, permanent mount failure, security impact or loss of user work is established; a constructor
failure already prevents that mount.

Proposed basis: Retain the in-range test-fidelity defect, consolidate g135/l2, and explicitly distinguish the pre-range
production diagnostic weakness. Repair should exercise the production view/provider boundary or a shared view factory,
not copy another separately safe getter.

Existing parent disposition (event 8): duplicate. Parent read complete claims085 and canonical g083/l1 plus g135/l2.
Exact same failed-mount test/getter counterexample, not another source defect: changed test safely returns host.element
while production dereferences unassigned created via getGrid().element before rethrow. Preserve
optional-chaining-inside-getter limit, original error masking, unchanged pre-range production structure and same-tip
test. One other-priority item; no observed runtimefailure, extraresourceleak/permanentmountloss/security/workloss. All
introduced fixture provenance and prior narrowedseverity retained.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g136 record](records/review-through-00de0b6c/groups/g136.json). Record ID: `l1`.
