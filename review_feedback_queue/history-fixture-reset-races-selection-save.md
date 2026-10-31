# History fixture reset can race an automatic selection save

Reviewed commit: 56dd4d2d9a9d16a182256c73d8aad2c9e06188e9

## TLDR

The history regression's server reset can be overwritten by an earlier automatic selection save, making its next
assertion depend on request ordering. The actual interleaving has not been reproduced.

## Details

`e2e/tests/sidebar.spec.ts:4131–4141` waits for the selected title before resetting preferences through the API. The
title observes local selection, not acknowledgment of its queued PUT. That automatic write and the test's direct API
reset are independent requests, so the older automatic save can commit after the reset. Establish the save's completion
or otherwise own this boundary before replacing server history. This is separate from the product's coalescing of
intermediate visits: here an already issued write races fixture reset.

Suggested bucket: **other**, for test reliability. Confidence: **likely**, with the request boundary established and the
adverse ordering unobserved. No product security or user-work-loss consequence is alleged. Proposed drop: none.

Possible cover: no exact acceptance, Planned, BUGS or queue match was found. FILTER's rare safely retryable-operation
rule may apply if this is rare, but rarity has not been established; the filters do apply to this committed-test review.
The latest-wins product preference contract explains the application write, not a settled test fixture. Provenance:
`session-history-tests-1/f1`, trailing review through `fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`. No runtime
reproduction was performed.

Original finding IDs: `session-history-tests-1/f1`.

## Additional review at d6645f77

Additional reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

Documentation-capture paths add the same independent API-restoration versus queued GUI-selection-write race. Source
investigation checked both incoming values and Playwright page lifetime: an already admitted older selection save can
outlive context disposal, while a still-unsent correcting save can die with that page. An adverse final preference
value/order remains unobserved, and the ordinary successful correction may restore the initial value. Suggested bucket
stays other, investigated uncertainty for the additional capture schedules; no product user-work loss is claimed.

Source and provenance, run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`:

- `g073/f1`: `e2e/docs-shots/visual-review.spec.ts:2641-2644,3289-3296`.
- `g075/f1`: `e2e/docs-shots/visual-review.spec.ts:530-551`.

Proposed drop: none for the retained remainder. No runtime reproduction, fix or triage decision was performed.
