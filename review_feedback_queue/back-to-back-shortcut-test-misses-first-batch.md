# Back-to-back shortcut test does not require its first batch

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

The back-to-back shortcut regression can pass even if the first batch is dropped.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The retained assertions do not distinguish both batches applying from only the later batch producing the checked state.
This is a source-confirmed oracle gap, not evidence of a production FIFO or replay failure.

Source locations: `e2e/tests/launcher-template-shortcuts.spec.ts:64-67,76-81,99-120`. Historical revision qualifiers in
those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Synthesis verified source finding. Reviewed d6645f771bee8ed03a66486ef8755834937d759d. TLDR: the browser regression
described as proving both queued shortcuts apply in order can pass when the first shortcut is fetched but never applied.
Seed already supplies kind=agent and agent=codex; first bolt repeats those and supplies effort=high/name=from bolt;
second supplies kind=agent/effort=low/name=from number. The final name and effort erase the first batch's unique
effects; Codex was already seeded. heldRead proves a request started, not application. Search/focus/no-extra-create
assertions also do not distinguish dropping the first application. A valid unique field from the first batch that
survives the second would provide the missing witness. Suggested bucket other; confidence confirmed source/assertion
gap, no production defect or runtime false-pass experiment claimed. proposed_drop false; possible_cover none exact after
searches across SPEC.md, SPEC_impl.md, TODO.md Planned (empty), BUGS.md, plans/queue, INDEX.md/all queue text,
TRIAGE_OUTCOMES.md. Exact near matches read: TRIAGE_OUTCOMES.md:17199-17216 only authorizes cleanup;
plans/queue/restart-with-template-refusal.md:86-108 concerns held-field/host-refusal tests. Neither addresses
first-batch effect witnessing. SPEC.md:853-861 and SPEC_impl.md:6118-6125 require ordered presses/batches. Provenance
caveat: immutable base da9d7411fd5c69255ea6d0b857a6a3aa48bd0c6c:e2e/tests/launcher-template-shortcuts.spec.ts:52-120
already contains the same fixture/assertion weakness (labels later change case). This is pre-existing evidence weakness
exposed by synthesis, not a regression introduced by assigned commit 18faeccb3c5a096441bb1d9d2d55cc91036f07e1. It stays
within the requested caller/test contract review; parent may scope the disposition accordingly. No FILTER clause matches
a deterministic test observable gap; the tests were not run.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g158/f6`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
