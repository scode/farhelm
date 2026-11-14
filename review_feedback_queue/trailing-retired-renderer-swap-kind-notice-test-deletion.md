# Retired renderer compatibility tests removed with the compatibility path

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Retired renderer compatibility tests removed with the compatibility path

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g179/c1`.

### g179/c1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 9 (candidate, open):

Reviewed commit: f560058da19e02716e6f943883a9508b447aff4e. Discovery interface synthesis: preserve all prior c1 source
references, original suspected consequence (loss of supported compatibility regression tests), proposed-drop bases and
checks. No drop is accepted. The supplied g082/c2 and g083/c5 records identify a wider production consequence of
old-page compatibility removal: valid pasted GridInput-shaped JSON may be reinterpreted as program input. They reference
g196/l1 for consolidation. That route is not refuted by correct new-page grid mounting, and the reload acceptance must
not be generalized to accept command reinterpretation. The exact original acceptance is SPEC_impl.md, Grid streams: "A
page loaded from the previous helm and left open across the update must be reloaded; nothing keeps it working." It
describes misdrawing and malformed input, not successful command reinterpretation. c1 remains a test-deletion candidate
about removed renderer/swap/pending/notice cases; its proposed drop is limited to those retired mechanisms.
Could-be-highest: no established highest route from the test-only removal; the supplied wider production route is
explicitly potentially highest and remains unresolved in the supplied interface records. Confidence: confirmed scope
limitation by source/contract already read; production route confidence and checks belong to the supplied records and
are not independently claimed here. Suggested bucket: other for this test candidate; possible_cover: supplied g082/c2,
g083/c5 and their g196/l1 reference for the wider compatibility concern. Caveat: current discovery will inspect only
assigned test changes and their replacement/oracle contracts, not broaden into a duplicate production review. No runtime
execution.

Returned proposal (claims-111, event 9): **covered**, suggested bucket **other**.

The narrow deletion concern targets renderer swap, terminal-kind selection, pending-kind and stream-notice compatibility
mechanisms that the all-grid protocol expressly removes. This acceptance must not be extended to successful old-page
command reinterpretation.

Original stronger consequence: Preserve loss-of-supported-compatibility-test concern and the interface caveat: valid
pasted GridInput-shaped JSON may be reinterpreted as program input according to other supplied records. Correct new-page
mounting and reload acceptance do not refute that wider potentially highest route.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Cover only deletion of retired renderer/kind/swap/notice regression cases. Independent check must
preserve the boundary against accepting successful command reinterpretation or dropping another owner's production
finding.

Existing parent disposition (event 9): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g179 record](records/review-through-00de0b6c/groups/g179.json). Record ID: `c1`.
