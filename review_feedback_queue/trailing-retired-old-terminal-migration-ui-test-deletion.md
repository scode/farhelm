# Removing the old-terminal migration UI tests matches the retired feature contract

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Considered whether deleting all migration-band browser tests leaves users without a required
warning or route to move running terminals. The removed tests exercise a compatibility UI the current contract
deliberately retires.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g180/c1`.

### g180/c1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 3 (candidate, open):

Reviewed commit: f560058da19e02716e6f943883a9508b447aff4e

TLDR: Considered whether deleting all migration-band browser tests leaves users without a required warning or route to
move running terminals. The removed tests exercise a compatibility UI the current contract deliberately retires.

Details: The entire 252-line file is deleted. SPEC.md:3002-3017, Terminal experience, explicitly says: "Every terminal
is new: it runs in a terminal host of its own (above)." It then says: "This release ends them: the first time a host's
supervisor of this release starts, it ends that tmux server, and with it everything that still ran in an old terminal,
without needing a tmux program." The corresponding replacement is present: service/core.rs:6142-6151 invokes
OldTerminals::end_private_server before reloading sessions, while service/core/old_terminals.rs:1-85 documents startup
conversion, cleanup and accepted residual failure behavior. Retaining the old warning/dismissal/kind-label tests would
assert retired behavior. No independent defects in the deleted implementation are being reviewed. Suggested bucket:
other (test coverage concern only). Confidence: confirmed by source and specification. proposed_drop: the removed
migration UI is expressly retired by SPEC.md:3002-3017, quoted above. possible_cover: none searched. Could-be-highest:
false for this test deletion; the specified termination itself is outside this candidate. Source-only conclusion; no
runtime behavior observed.

Returned proposal (claims-111, event 3): **covered**, suggested bucket **other**.

The removed migration-band/warning/dismissal/kind-display tests assert a UI for terminals the revised specification
deliberately ends at startup. The new startup behavior is present. This is limited acceptance of retirement, not
approval of an independently defective termination implementation.

Original stronger consequence: Deleting migration UI tests could leave users without a required warning or route to move
running terminals. The specified ending itself can terminate user processes but was expressly outside this test-deletion
candidate; retain that boundary and do not infer approval of another termination defect.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Cover only the retired warning/migration UI tests under the explicit product contract. Independent drop
check should verify the user-process boundary rather than treating all old-terminal consequences as accepted.

Existing parent disposition (event 3): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g180 record](records/review-through-00de0b6c/groups/g180.json). Record ID: `c1`.
