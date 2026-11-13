# Renderer removal leaves hidden-pane rationale pointing at removed FitAddon

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Renderer removal leaves hidden-pane rationale pointing at removed FitAddon

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g079/l2`.

### g079/l2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 12 (finding, open):

Reviewed commit: f560058da19e02716e6f943883a9508b447aff4e

## TLDR
The intermediate grid-only release still explains hidden terminal sizing through a component it has removed. Its zero-column explanation does not describe the grid's retained-size/no-proposal behavior, giving maintainers the wrong model for the visibility rule. A later assigned commit corrects it.

## Details
At app.css:8571–8580 the rationale for visibility rather than display:none still says ‘FitAddon.fit() would size an unselected terminal to zero columns’. This commit removes the FitAddon asset registration and injection from lib.rs (VENDOR_FIT_JS) and removes its construction/use from terminal.js, leaving no FitAddon runtime for this comment to describe. SPEC_impl.md:1835–1865 defines the replacement as the grid's deterministic fit, not an add-on. Actual grid-terminal.js:462–496 documents that an unmeasurable container proposes nothing and returns before assigning fitted on nonfinite dimensions; :161 and :404–407 initialize that retained size to 80x24. Production sizing can be correct while this independent maintenance explanation remains stale. Fix direction was in fact implemented later: 61e632323dbc6b4a35ed27bf5b9833102ffd5a4e app.css:8571–8575 changes the rationale to the grid being unable to propose a fit without a layout box. The discovery finding is therefore historical/intermediate and does not request another fix at the pinned endpoint.

Whole-range attribution checked: exclusive base 6e423e30068b62c6cfa276c6d720ee35bb70387a app.css:8630–8634 and immediate parent app.css:8611–8615 already contain this sentence, but both still ship and invoke FitAddon. f560058 removes that live referent while preserving the rationale in its assigned stylesheet. The claim that zero columns accurately described the old add-on is not newly validated or asserted here; the in-range defect is failing to replace the explanation when that implementation is removed. This is distinct from supplied g182/c1's separately changed test comment about a two-column floor. Suggested bucket: other, internal documentation accuracy only. Could-be-highest: no established or suspected security/loss-of-work route from this comment. Confidence: confirmed through introducing removal, current and base source, replacement contract and later corrective patch; no execution. proposed_drop: false for the reviewed intermediate revision. possible_cover: source-confirmed correction at 61e632323dbc6b4a35ed27bf5b9833102ffd5a4e, within the assigned range; no prohibited coverage sources searched. No missing premise or runtime reproduction.

Returned proposal (claims-052, event 12): **needs-parent**, suggested bucket **other**.

The stale explanation is a real in-range maintenance-documentation defect at f560058, independently of correct sizing. Commit 61e63232 corrects it before the review endpoint. It is neither out-of-range nor source-refuted at its reviewed revision; the result schema has no resolved-in-range disposition.

Original stronger consequence: Maintainers are taught a removed component and the wrong sizing behavior as the reason hidden panes need layout boxes. Other priority; no security or user-work-loss consequence was suspected.

Unresolved premise or limitation: No factual premise remains for the historical defect or its correction. Parent must choose its bookkeeping treatment for an established defect resolved within the reviewed range.

Proposed basis: Historical defect confirmed; corrected by 61e63232 before endpoint. Preserve both facts and do not request another code fix. The supplied g182/c1 two-column test-comment issue is distinct.

Existing parent disposition (event 12): rejected. Parent read full claims052. Removing FitAddon at f560058 makes app.css8567-8580's old hidden-pane rationale inaccurate, independently of the correct new grid sizing. The later61e63232 comment explicitly replaces the removed-component rationale and endpoint8571-8575 retains it. Historical defect confirmed and corrected within range, so preserve both in audit without an obsolete queue item. No higher consequence or unresolved premise; separate g182/c1 two-column explanation not covered.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g079 record](records/review-through-00de0b6c/groups/g079.json). Record ID: `l2`.

