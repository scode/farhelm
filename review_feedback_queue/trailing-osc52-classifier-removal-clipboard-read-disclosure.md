# Removing the raw OSC 52 classifier does not expose clipboard reads

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: removing the browser raw OSC 52 classifier does not permit terminal programs to read the viewer clipboard.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g186/c2`.

### g186/c2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 10 (candidate, open):

Reviewed commit: f560058da19e02716e6f943883a9508b447aff4e. Identity correction requested by caller: restore c2 to the original clipboard-confidentiality candidate and preserve every prior event. The distinct primary-guidance defect previously placed under c2 is tracked separately as linked f1; it is not dropped by this revision. TLDR: removing the browser raw OSC 52 classifier does not permit terminal programs to read the viewer clipboard. Details: farhelm-terminal-emulator/src/effects.rs:177-188 turns ClipboardStore into effects but discards ClipboardLoad; grid-terminal.js:1983 forwards completed model writes only and terminal.js:4326-4330 only calls a write sink. Original contract: SPEC.md Terminal experience says 'an OSC 52 READ is never answered with clipboard contents'. Suggested bucket for the considered disclosure: highest/security. could-be-highest: yes, clipboard secrets would be exposed if a read route existed; source refutes that route. Confidence: confirmed source refutation, no execution. proposed_drop: source refutation of the original confidentiality concern only, subject to independent checking; not an accepted parent drop. possible_cover: supplied g188/c3 corroborates the original boundary, without replacing this record or a coverage search. Prior consequence preserved separately: primary-selection writes fail to suppress drag-copy guidance, an other-bucket correctness issue linked as f1; earlier c2 lead/finding events remain valid evidence of that distinct cause. Base-range attribution is being checked under f1, not generalized from the immediate-parent check.

Returned proposal (claims-117, event 10): **refuted**, suggested bucket **other**.

The remaining callback receives completed clipboard writes. Removing a classifier for the deleted raw terminal path does not create a viewer-clipboard read route. The primary-selection guidance defect in earlier c2 events remains separate under g186/f1.

Original stronger consequence: Highest/security: a terminal program might obtain clipboard secrets after isOsc52Write removal. Initial c2 also considered false guidance suppression. Later primary-write findings under c2 were split into f1, not withdrawn as incorrect. The confidentiality refutation remains subject to independent checking.

Unresolved premise or limitation: A read request reaching a viewer-clipboard read sink is absent on the checked event/callback path. This does not constitute a new audit of all escape parsing.

Proposed basis: Drop only the classifier-removal confidentiality candidate after independent agreement. Preserve c2 lines 5-7 as provenance of g186/f1; do not use this refutation to dismiss primary guidance. The other bucket records that no surviving confidentiality route is established, not authorization to lower or drop it.

Existing parent disposition (event 10): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g186 record](records/review-through-00de0b6c/groups/g186.json). Record ID: `c2`.

