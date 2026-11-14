# suspended-source Clone rules distinguish agent authority from GUI prefill

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Considered whether the new requirement to refuse an agent Clone while its environment source is
suspended contradicts the product promise that Clone remains available on a suspended sandbox. The complete caller
scopes distinguish those operations, so no separate contract defect is established.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g005/f6`.

### g005/f6

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 15 (candidate, open):

Reviewed commit: b106c0e49646ecb9a17e1a06b17a70da0ece73e0. TLDR: Considered whether the new requirement to refuse an
agent Clone while its environment source is suspended contradicts the product promise that Clone remains available on a
suspended sandbox. The complete caller scopes distinguish those operations, so no separate contract defect is
established. Details: the new Agent requests and environments subsection at SPEC_impl.md:6678-6685 requires the agent's
clone plan to read launch and repository intent live from the source supervisor, explicitly refusing suspended or
unreachable sources. SPEC.md:1545-1550 keeps the GUI's Clone control available during suspension; SPEC.md:1487-1494
describes that control targeting a fresh environment. This is not a promise that an agent can obtain live source
authority without resuming it. The complete Agents and environments subsection at SPEC.md:4289-4341 promises the
environment placement and approval rules but no suspended-source availability. In the same reviewed tree,
session_view.rs:2641-2648 opens a new-session form through HeaderPrefillRequest::Clone(header_session.clone()), rather
than dispatching an agent Clone. SPEC_impl.md:3928-3933 separately requires ordinary agent clone to read live source
data instead of the helm cache. Thus 'as ordinary clone reads them' in the agent subsection has a caller-specific
referent and need not imply that the GUI prefill must perform that agent operation. Suggested bucket: other if wording
clarification were desired; could-be-highest: false, no security or loss-of-work route identified. Confidence: confirmed
source/contract distinction; no runtime availability observed. proposed_drop: exact caller and live-source contract
distinction above; no actionable contradiction established. possible_cover: none in supplied neighbour context; no
prohibited coverage search. This is a newly considered candidate, not a verified or unverified defect, and no parent
disposition is presumed. No tests, builds, reproductions or live actions.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g005 record](records/review-through-00de0b6c/groups/g005.json). Record ID: `f6`.
