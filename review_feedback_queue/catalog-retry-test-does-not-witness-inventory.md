# Catalog retry test does not witness recovered inventory

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

The catalog retry regression can pass without proving that the retried catalog response was consumed.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

Request counting and a cleared error do not distinguish recovered inventory from inherited state; the inspected mocked
restart refusal can still satisfy the later assertions. Existing closed-model Enter accounting is a separate
already-covered oracle. This finding concerns test detection, not a demonstrated production catalog defect.

Source locations: `e2e/tests/restart-with.spec.ts:2254-2267`. Historical revision qualifiers in those references are
retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Provenance clarification after final f3 investigation event: immutable base
da9d7411fd5c69255ea6d0b857a6a3aa48bd0c6c:e2e/tests/restart-with.spec.ts:2235-2268 has the same oracle. This is a
pre-existing test weakness preserved in the assigned 08aaac8a4a5bcf91726a1d3bfcfc980de4116971 label update, confirmed
still present at d6645f771bee8ed03a66486ef8755834937d759d. It is not attributed as a sentence-case production
regression. All evidence, suggested bucket other, confidence confirmed source-level oracle gap (unexecuted
counterexample), proposed_drop none and possible_cover none from the preceding f3 event remain unchanged. No operational
actions or experiments ran.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g036/f3`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
