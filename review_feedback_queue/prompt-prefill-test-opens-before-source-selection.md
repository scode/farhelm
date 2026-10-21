# Prompt-prefill test opens before its source selection settles

Reviewed commit: 56dd4d2d9a9d16a182256c73d8aad2c9e06188e9

## TLDR

The prompt-prefill browser regression can open New before its intended source session is selected, capturing the wrong
starting folder and failing for setup timing. The startup interleaving has not been reproduced.

## Details

`e2e/tests/templates.spec.ts:886–887` pins the source in server preferences, then calls `openNew`. That helper clicks as
soon as New is ready (`603–608`), without observing the asynchronous initial selection. The dialog captures its source
folder once (`crates/farhelm-ui/src/list/view.rs:3432–3450`); a later selection update does not reseed it. Without a
selected source it starts at `~`, while the intended fixture lives elsewhere. Observe the source as selected before
opening New, retaining the prompt-prefill assertion. The new pre-pick read widens an existing setup dependency.

Suggested bucket: **other**, for test reliability. Confidence: **likely**, with the missing selection premise and
capture boundary established but adverse scheduling unobserved. No security or user-work loss is alleged. Proposed drop:
none.

Possible cover: no exact Planned, BUGS, queue or accepted specification/ledger match was found. A rare safely retryable
fixture failure could match FILTER, but rarity remains unestablished; tests are not categorically exempt from filtering.
Provenance: `session-history-tests-1/f3`, trailing review through `fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`. No runtime
reproduction was performed.

Original finding IDs: `session-history-tests-1/f3`.
