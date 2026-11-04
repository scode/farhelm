# Drag-copy guidance keeps its old gap after terminal text-size changes

Reviewed commit: 4e39472e73711eba694854e921b1f77459092188

## TLDR

Changing terminal text size while drag-copy guidance is visible leaves the hint at its old pixel position. Its gap no
longer equals the promised three current terminal rows.

## Details

`crates/farhelm-ui/assets/terminal.js:3020–3051` changes grid/xterm metrics and fits the terminal without notifying the
notice controller. `grid-terminal.js:1938–1942` remeasures, but does not reposition the notice. A text-size shortcut or
terminal zoom button need not resize the window.

The geometry closures at `terminal.js:5380–5388,5923–5926` can read the current row height. `copy-on-select.js:251–281`
computes placement from that geometry, but it is invoked on show at `299–305` and on the window-resize listeners at
`terminal.js:5443,6059`. The unit test manually calls reposition after changing fake geometry, so it does not exercise
the missing shipped text-size notification.

`SPEC.md`, “Terminal experience,” 2296–2302 promises the three-row gap and a 30-second lifetime with explicit dismissal
boundaries. `SPEC_impl.md:1648–1658` describes geometry-based placement. The report at
`7a7c160b124ac270f9514642563080dcf36bce44:plans/reports/grid-drop-and-drag-notice.report.md:20–23` says the gap follows
terminal text size, which this caller path does not maintain.

Notify the notice controller after metric changes and recompute placement **without resetting its original expiry**. Do
not hide it merely because the font changed: that would add a dismissal boundary absent from the specification.
Verification should show the hint, change text size without a window resize, check the three-row gap at the new metrics,
and confirm expiry still uses the original showing time.

## Confidence and priority

Confidence: confirmed source path, without runtime reproduction. The placement arithmetic is correct when invoked. The
three involved assets remain unchanged through `c5ae8548439cc5bfa33c11ef7ce8da6129d8072a`.

Suggested bucket: `other`, for transient positioning. No security or work-loss route is established.
`could_be_highest: false`. `proposed_drop: null`.

Possible cover: no exact matching acceptance, planned repair, known bug, queue item or ledger decision was found in the
required recording sources at the review tip or recording-time main. The cited related concerns have distinct triggers
or consequences unless explicitly grouped here. No full FILTER.md match was established.

Grouped findings: `g055-l2` is the executable finding, `g054-s3` supplies controller-side evidence and `g039-l5` records
the report-side observation of this same defect.

## Provenance

Review run `ffa3b94e-5eb1-4569-8eb5-1a52b62110d1`: owner `g055`, ID `g055-l2`, latest `g055/findings.jsonl` physical
line12; owner `g039`, ID `g039-l5`, latest `g039/findings.jsonl` physical line16; grouped controller finding `g054-s3`.
Their latest records explicitly preserve the original lifetime and do not propose hiding the notice on a font change.
