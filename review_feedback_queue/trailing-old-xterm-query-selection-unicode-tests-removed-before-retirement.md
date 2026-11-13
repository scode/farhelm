# Old-renderer regressions lose coverage before the supported renderer is retired

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: At this intermediate revision, existing sessions still use the old terminal renderer, but its query-reply, selection-dismissal and emoji regression checks are deleted. The later removal of the renderer ends the current need for those tests; it does not show the intermediate loss was harmless or that grid tests covered the old code.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g178/g178-l1`.

### g178/g178-l1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 8 (finding, open):

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513. TLDR: At this intermediate revision, existing sessions still use the old terminal renderer, but its query-reply, selection-dismissal and emoji regression checks are deleted. The later removal of the renderer ends the current need for those tests; it does not show the intermediate loss was harmless or that grid tests covered the old code. Details: SPEC.md:3016-3034 still promises that existing old terminals remain until the user moves them. The implementation still constructs xterm at crates/farhelm-ui/assets/terminal.js:3900, suppresses duplicate DECRQM replies at :4120-4128 and DECRQSS at :4144-4147, and clears both old-renderer/native selections through :4250-4295. These are precisely the safeguards exercised by the deleted terminal.spec.ts tests. The removed Unicode tests at parent revision e2e/tests/terminal-unicode.spec.ts:123-215 separately inspect wide/continuation cells, cursor-addressed redraws and the adjacent colored cell geometry in xterm's model/DOM. The same commit deletes e2e/suite-terminals.ts and removes the legacy test-run selection from playwright.config.ts; its deleted suite instructions explicitly said old terminals remain for sessions made by earlier versions and kept implementation-specific tests in their own run. Thus removal of the fixture and tests precedes removal of supported production paths. The independent grid geometry/selection tests at e2e/grid-client/grid-terminal.spec.ts:199-371 and :1046-1079, plus real-program grid Unicode replay at terminal-unicode.spec.ts:109-192, remain useful grid-specific observables; they neither call nor refute regressions in the retained xterm handlers. Cross-batch scope: f560058da19e02716e6f943883a9508b447aff4e later removes the old renderer/protocol branch, is an ancestor of pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db, and its SPEC_impl.md:1842-1843 says every terminal mounts the grid. Pinned-tip source has no new Terminal, swallowDecrqm or registerDcsHandler in terminal.js and no vendor asset subtree; dismissSelectionSoon still appears as a generic attachment callback parameter, not the old implementation. Therefore this is a confirmed historical coverage gap, not a claim the pinned tip should restore xterm or has a current old-renderer bug. Suggested bucket: other, lost regression coverage in maintainer tooling; no new product malfunction or security/loss-of-work route established. Could-be-highest: no established route. Confidence: confirmed source/patch/contract analysis; no execution observation. proposed_drop: false. Preserve prior proposed drop verbatim in history: 'no actionable defect demonstrated by removing renderer-specific tests; source refutation is the split of query ownership and the surviving independent grid assertions above.' Synthesis rejects that rationale: absence of a new malfunction does not refute useful deleted regression tests, and grid ownership is not old-renderer coverage. Later complete retirement is a separate possible disposition reason for the parent, not an accepted drop; parent has accepted none. possible_cover: none; no queue or excluded-coverage search. No readily checkable premise remains open for this record.

Returned proposal (claims-111, event 8): **retain-verified**, suggested bucket **other**.

Useful query-reply, selection and emoji regressions are deleted while their old-renderer production paths remain supported. Grid-specific tests cannot cover those retained paths. Later complete removal ends the present need for them without disproving the historical gap.

Original stronger consequence: Lost old-renderer query-reply, selection-dismissal and emoji protection at the intermediate revision. Preserve the rejected earlier rationale: 'no actionable defect demonstrated by removing renderer-specific tests; source refutation is the split of query ownership and the surviving independent grid assertions above.' That rationale does not refute useful deleted tests. No current xterm bug is asserted.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Retain the source-established historical test defect as other, with later retirement explicit. Parent decides historical obsolescence separately.

Existing parent disposition (event 8): rejected. Full111:647 removesoldbrowserqueryreplysuppressionwithpositiveparsercontrol,local/nativeselectiondismissal,andwideemoji/model/DOMgeometrytests whileactualoldDECRQM/DECRQSS/browserrenderercontinues. Independentgridqueries/paintersdon'texerciseoldretainedpaths; initialsplitownership/noactionablebug dismissalwithdrawn. f560retiressubjectbytip; preserveactualintermediategap andno obsoletecurrentfixturefix or inferredunsafeinput/workloss.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g178 record](records/review-through-00de0b6c/groups/g178.json). Record ID: `g178-l1`.

