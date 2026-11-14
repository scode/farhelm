# Grid geometry test gives a false two-column failure rationale

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: The revised regression-test explanation tells maintainers that an unmeasured hidden
terminal falls to two columns, although the grid retains its default or previous size when it cannot propose a fit. This
misstates what the size assertions prove.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g182/c1`.

### g182/c1

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 14 (finding, open):

Reviewed commit: f560058da19e02716e6f943883a9508b447aff4e. TLDR: The revised regression-test explanation tells
maintainers that an unmeasured hidden terminal falls to two columns, although the grid retains its default or previous
size when it cannot propose a fit. This misstates what the size assertions prove. Details:
terminal-tabs.spec.ts:1312-1313 newly replaces 'The floor is xterm.js’s own minimum' with 'A fit that measured nothing
lands on the grid’s floor (two columns)'. At this exact revision grid-terminal.js:161 defines DEFAULT_SIZE as 80x24;
:404-407 initializes fitted from it; :473-488 returns without assigning fitted if font metrics cannot be obtained or
dimensions are non-finite. :494-496 changes fitted only after a valid proposal. Therefore an absent measurable pane need
not yield two columns: it can leave 80x24 or an earlier proposal, both potentially above the test’s >10/>4 thresholds.
The test’s own :1305-1306 comment already says a hidden pane that measured nothing would leave size unchanged.
Whole-range attribution: exclusive-base 6e423e30068b62c6cfa276c6d720ee35bb70387a terminal-tabs.spec.ts:1435-1436
describes the xterm minimum instead; git blame and the f560058 diff establish the new false grid rationale was
introduced in-range. The later assigned 61e632323dbc6b4a35ed27bf5b9833102ffd5a4e fixes the separate top-level FitAddon
explanation but leaves this two-column statement at :1313-1314. Discovery also checked pinned tip
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db:1313-1314: the statement remains. Suggested correction: say the grid-size
bounds establish a usable retained/proposed grid, and retain actual element geometry as the evidence that the hidden
pane has a layout box. Contract read at the introduction revision: SPEC_impl.md:1835-1887, The grid client, describes
measurement-based proposals and unchanged observations proposing nothing; grid-terminal.js:462-467 explicitly says 'A
container with no layout box, or no font metrics yet, proposes nothing.' Suggested bucket: other, inaccurate test
documentation. Confidence: confirmed by exact introduction-revision source and history, not execution. proposed_drop:
false; prior immediate-parent scope exclusion is superseded by whole-range attribution and no drop was accepted.
possible_cover: none; no prohibited coverage sources searched. Could-be-highest: no; no product security or loss-of-work
route. Preserve separate oracle refutation and original limited consequence: :1310-1311 independently assert actual
hidden width and height >0 and the later shell witness checks usability, so the false comment does not establish that
this test passes with display:none or that runtime sizing is broken. No reproduction or runtime test performed.

Returned proposal (claims-113, event 14): **retain-verified**, suggested bucket **other**.

The newly written test comment gives a false explanation of grid sizing. This is an in-range test-documentation defect;
the independent pixel-geometry assertions remain effective. No product sizing failure or false pass was established.

Original stronger consequence: Earlier records considered an implied weakened geometry oracle, then explicitly refuted
it with positive bounding-rectangle assertions and the later shell witness. The retained consequence is misleading test
documentation only. Earlier immediate-parent scope drops were superseded by whole-range attribution; no drop was
accepted.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Retain the narrow verified explanation defect. Bucket other: harm is confined to maintainer
understanding of a test, with no security or user-work-loss route.

Existing parent disposition (event 14): verified. Parent explicitly accepted named retain-verified proposal from
claims-113 after inspecting it. The newly written test comment gives a false explanation of grid sizing. This is an
in-range test-documentation defect; the independent pixel-geometry assertions remain effective. No product sizing
failure or false pass was established. Retain the narrow verified explanation defect. Bucket other: harm is confined to
maintainer understanding of a test, with no security or user-work-loss route. Checked source/results:
["f560058da19e02716e6f943883a9508b447aff4e:e2e/tests/terminal-tabs.spec.ts:1305-1315 says an unmeasured fit leaves size
unchanged, then incorrectly says it lands at two columns. The commit diff replaces the xterm minimum rationale with that
grid-specific
claim.","f560058da19e02716e6f943883a9508b447aff4e:crates/farhelm-ui/assets/grid-terminal.js:159-161,404-407 initializes
fitted size to 80x24. fit at 473-496 returns without replacing fitted when font measurement is unavailable or pane
dimensions are non-finite. Thus failure to propose a fit need not produce two columns.","The introduction revision's
SPEC_impl.md:1835-1887, The grid client, specifies measurement-based proposals. This supports the correction; it does
not accept inaccurate test
commentary.","f560058da19e02716e6f943883a9508b447aff4e:e2e/tests/terminal-tabs.spec.ts:1310-1311 independently requires
positive actual element dimensions; 1317-1320 checks shell usability after reveal. Those guards refute the implied
stronger geometry-oracle failure.","git merge-base --is-ancestor succeeded for exclusive base -> f560058 and f560058 ->
inclusive tip. The same erroneous comment remains at
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db:e2e/tests/terminal-tabs.spec.ts:1313-1314. Earlier assigned-patch scope
exclusions are superseded, not silently discarded.","Source inspection only; no runtime reproduction or test was
performed."] Current coverage/results: ["SPEC.md searched for hidden panes, fitting and geometry; no acceptance of this
explanation. SPEC_impl.md, The grid client (1836-1888), read in full: fit formula is context, not coverage of the false
comment.","Current TODO.md Planned bucket is empty (25-27). BUGS.md's sole SSH-transfer hang entry does not
match.","plans/queue searched for fitting, hidden geometry and terminal-tabs; tmux-drop-from-docs criteria 3/5 concern
removal of stale xterm rationales, but plans/REPORTS.md:32 says it landed on 2026-11-13. It supplies no unlanded-plan
coverage, and the new grid-only error survives it. Other geometry hits concern different tests or UI
geometry.","INDEX.md read in full and queue text searched for the exact no-measurement/two-column claim and
terminal-tabs source. No exact item found. Existing switch-geometry and font-measurement items concern different
triggers and assertions, not this rationale.","Active TRIAGE_OUTCOMES.md searched for the actual claim, hidden panes,
fitting and terminal-tabs; no matching decision. Archive not read.","FILTER.md read in full: no filter covers an
unconditional inaccurate test explanation. The rare-diagnostic filter requires a rare trigger, which this claim lacks."]
Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "Earlier records considered an
implied weakened geometry oracle, then explicitly refuted it with positive bounding-rectangle assertions and the later
shell witness. The retained consequence is misleading test documentation only. Earlier immediate-parent scope drops were
superseded by whole-range attribution; no drop was accepted."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g182 record](records/review-through-00de0b6c/groups/g182.json). Record ID: `c1`.
