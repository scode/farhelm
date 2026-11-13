# Intermediate loss of sustained viewport regression checks before old-renderer retirement

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: while existing old terminals remain supported, their tests lose the check that scrolled-back rendered rows stay consistent throughout continuing output and repeated pane resizes. This is a confirmed historical test-coverage defect, independently of whether a new production failure was introduced. Normalize prior c2 candidate to finding as requested by parent/c2-record-kind; no new discovery is claimed by this record-kind correction. Preserve original wording: 'Deleted viewport-freeze suite targets the retired creation path and xterm workaround.' Preserve all original consequences: pre-cap/full-cap continuous output, DECSTBM and synchronized-output viewport correctness, and sibling-band resize coverage disappear; the intermediate old renderer still needs them. Original no-production-defect/commit-acknowledgment dismissal is superseded, not accepted. Source basis remains complete 759-line deletion patch; every viewport sample uses island.term.buffer.active and .xterm-rows; tests additionally require scrolledRefreshCount to advance. SPEC_impl.md at reviewed revision:1528-1540 records the vendored stale-paint failure and compensating refresh. Actual retirement before pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db: dbfc69751215dbf15e1d31d5bed73c3754c4ebc4 replaces adoption with OldTerminals::end_private_server before reload; f560058da19e02716e6f943883a9508b447aff4e removes buildXtermSurface, writeBytes/refreshIfScrolledBack, workaround state and scrolledRefreshCount, and always mounts grids. Pinned SPEC_impl.md:2780-2795 states 'a starting supervisor ends that server' and '

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g177/c2`.

### g177/c2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 9 (finding, open):

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513. TLDR: while existing old terminals remain supported, their tests lose the check that scrolled-back rendered rows stay consistent throughout continuing output and repeated pane resizes. This is a confirmed historical test-coverage defect, independently of whether a new production failure was introduced. Normalize prior c2 candidate to finding as requested by parent/c2-record-kind; no new discovery is claimed by this record-kind correction. Preserve original wording: 'Deleted viewport-freeze suite targets the retired creation path and xterm workaround.' Preserve all original consequences: pre-cap/full-cap continuous output, DECSTBM and synchronized-output viewport correctness, and sibling-band resize coverage disappear; the intermediate old renderer still needs them. Original no-production-defect/commit-acknowledgment dismissal is superseded, not accepted. Source basis remains complete 759-line deletion patch; every viewport sample uses island.term.buffer.active and .xterm-rows; tests additionally require scrolledRefreshCount to advance. SPEC_impl.md at reviewed revision:1528-1540 records the vendored stale-paint failure and compensating refresh. Actual retirement before pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db: dbfc69751215dbf15e1d31d5bed73c3754c4ebc4 replaces adoption with OldTerminals::end_private_server before reload; f560058da19e02716e6f943883a9508b447aff4e removes buildXtermSurface, writeBytes/refreshIfScrolledBack, workaround state and scrolledRefreshCount, and always mounts grids. Pinned SPEC_impl.md:2780-2795 states 'a starting supervisor ends that server' and 'before sessions are reloaded'. Replacement observables checked in synthesis: grid-terminal.spec.ts:714-760 compares DOM rows across a history/frame update, :1186-1223 checks rendered line retention after cap eviction, :1297-1329 checks oldest-line paint after trimming. They explicitly flush, so they are not equivalent to naturally scheduled sustained flood/resize painting. grid-flood.spec.ts observes model snapshots, not viewport DOM. No surviving grid natural-paint failure is asserted; the concrete xterm workaround subject no longer exists at tip. Suggested bucket: other, useful maintainer regression coverage lost at an in-range intermediate revision. Could-be-highest: no established shipped security/loss-of-work route. Confidence: confirmed source/contract/patch analysis only; no runtime tests or reproductions. proposed_drop: preserve the prior proposal based solely on later source retirement of the exact subject, for independent parent disposition; this does not erase the historical defect or suggest restoring xterm at tip. No parent drop or lowering accepted. possible_cover: none; no prohibited coverage sources searched. Shared idle-policy loss remains separately recorded as l1. No unresolved source premise for c2's intermediate gap or retirement.

Returned proposal (claims-111, event 9): **retain-verified**, suggested bucket **other**.

The intermediate loss of useful rendered-viewport regression protection is established. Later removal of the tested renderer makes restoration at tip inappropriate, but does not refute the in-range historical gap. Preserve it for the parent's disposition rather than silently treating retirement as historical coverage.

Original stronger consequence: Preserve the lost pre-cap/full-cap continuous-output, DECSTBM, synchronized-output and sibling-band resize protection while old terminals remained supported. No grid natural-paint defect or newly introduced product failure was established. The earlier proposed retirement-based drop remains a proposal, not an accepted decision.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Record the historical test defect as other, explicitly stating that its concrete renderer no longer exists at tip. Parent decides whether historical obsolescence warrants a separate disposition.

Existing parent disposition (event 9): rejected. Full111:647 removes753line sustainedrenderedviewport oracle forpre/fullhistorycap eviction,DECSTBM,synchronizedoutput,siblingbandresize whileoldrenderer andstale-paintworkaroundstillserved. Usefulregressionprotectionlost, no-productbug/commitintentisnotrefutation. f560retiresconcretextermrendererbytip; don'trestoreobsoleteviewportworkaroundfixtures. Preservehistoricalgap, distinct178query/selection/unicodetests, no actualstaleviewport/gridbug.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g177 record](records/review-through-00de0b6c/groups/g177.json). Record ID: `c2`.

