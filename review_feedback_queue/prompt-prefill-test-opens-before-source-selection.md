# Prompt-prefill test opens before its source selection settles

Reviewed commit: 56dd4d2d9a9d16a182256c73d8aad2c9e06188e9

## TLDR

The prompt-prefill and clone-defaults browser regressions can open New before their intended source session is selected,
capturing the wrong starting folder and failing for setup timing. The startup interleavings have not been reproduced.

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

## Additional clone-defaults fixture

The same reviewed commit adds a preference pin at `e2e/tests/clone.spec.ts:941`, then waits only for row visibility at
`943–945`. After opening and cancelling Clone, the test opens New and expects the source folder (`964–971`). Neither the
menu helper (`helpers/fleet.ts:1290–1336,1349–1356`) nor the Clone callback
(`crates/farhelm-ui/src/list/view.rs:2603–2625`) selects the source. If startup's GET is still pending, New captures no
source (`view.rs:3432–3450`) and seeds `~` (`create_form.rs:2692–2702`). A later selection cannot reseed that opening.
Observe source selection before the first Clone/cancel/New sequence. A successful later clone selects its result, so
that later assertion is not the reported gap.

Suggested bucket remains **other**. Confidence: **confirmed conditional source mechanism**; adverse runtime ordering and
rarity remain unmeasured. No security or user-work-loss consequence is claimed. This adds a separate fixture needing its
own wait, not another queue item. No exact acceptance was found; the existing filter caveat above still applies.
Proposed drop: none for the added location.

Additional provenance: trailing-review run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, discovery `g068/f4`.
