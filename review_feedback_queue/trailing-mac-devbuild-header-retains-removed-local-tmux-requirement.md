# Intermediate Mac devbuild requirements still demand the removed tmux prerequisite

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: the Mac development-build instructions still say tmux is needed to run the app after this change removes that prerequisite, and falsely say the script checks it before building. This is a historical documentation defect; the pinned tip is corrected.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g093/l1`.

### g093/l1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 19 (finding, open):

Reviewed commit: 17a3fe858c5511a24b7403af15dd41c93bf69fd7. TLDR: the Mac development-build instructions still say tmux is needed to run the app after this change removes that prerequisite, and falsely say the script checks it before building. This is a historical documentation defect; the pinned tip is corrected. Details: scripts/devbuild-mac.sh:53-60 retains tmux in Requirements and says every listed item is checked; the assigned patch deletes the local tmux lookup/need call at former 411-420. The immediate parent's desktop.rs:1202-1214 still calls run_tmux_preflight_or_exit before startup, and this commit removes that app preflight, establishing why the retained header becomes wrong here. The separate static remote-Linux payload staging still present at this revision does not require a Mac-local tmux executable and cannot justify a local runtime requirement. Exact later repair: cb762a313abbe6da03ae0bf041f07486c4ed4df9's scripts/devbuild-mac.sh hunk at old lines 55-60 removes the tmux clause while retiring remote payload staging. The corrected Requirements wording remains at pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db:49-55. The complete in-range script history has the introduction and this repair, and the original parent still required the dependency. Suggested bucket: other (misleading maintainer development instructions, with no new runtime failure established). Confidence: confirmed by source, contract and exact historical repair patch, not execution. proposed_drop: false; preserve the intermediate misleading requirement, explicitly marked repaired in range. possible_cover: none in supplied interface findings; no prohibited coverage sources searched. Could-be-highest: no; no security or loss-of-work route. No unresolved premise remains for this narrow documentation issue.

Returned proposal (claims-061, event 19): **needs-parent**, suggested bucket **other**.

A real intermediate source-comment defect is established, but cb762a31 repairs it before the pinned tip. Preserve the historical finding and repair; parent should classify it as resolved-in-range rather than claim the tip still has the prerequisite.

Original stronger consequence: The Mac developer is falsely told to install tmux to run the app and that the script checks it. This is historical maintainer documentation in executable tooling, not a new product runtime failure. The reviewer explicitly preserves that the pinned tip is corrected.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Source-confirmed intermediate comment/contract mismatch, repaired by cb762a31. Parent to represent resolved historical evidence without inventing a remaining repair.

Existing parent disposition (event 19): rejected. Parent read full claims061.17a3fe85 removes local app/preflight tmux requirement, making inherited developer-script header53-60 newly false. cb762a31 corrects that header before tip;00de49-55 retains correction. Historical documentation defect preserved with both revisions, no residual item requiring fix. Remote Linux payload staging is a different prerequisite and not a runtime refutation of the original false local guidance. No higher consequence.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g093 record](records/review-through-00de0b6c/groups/g093.json). Record ID: `l1`.

