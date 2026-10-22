# A failed held preference reply can block fixture restoration

Reviewed commit: 2b775eff03705c9f3b2cf611c45a3cf93285752b

## TLDR

A failed held reply in four browser tests can leave cleanup waiting before it restores shared preferences or deletes
owned templates. Later tests reuse that stack and may inherit the changed state. The failure schedule has not been
exercised.

## Details

The preference-transfer cases in `e2e/tests/popover-template-movement.spec.ts:198–202` mark capture, then release a
separate retired gate only after successful `route.fulfill`. Cleanup waits on that gate at `213–218`, before preference
restoration and template deletion at `57–75`. `helpers/drained-route.ts:3–12,25–35` recognizes fulfillment channel
errors and retires callback work; removing/draining that handler at `39–42` does not settle the caller's separate gate.
`helpers/cleanup.ts:8–13` awaits cleanup callbacks sequentially.

The outer timeout bounds the wait, not restoration. The passive evidence fixture at `helpers/evidence.ts:31–65` provides
no independent restoration, and `e2e/playwright.config.ts:58–75,98–107` reuses stacks across specs. The analogous Rename
handler at `popover-template-movement.spec.ts:279–305` does not wait on this gate and is outside the finding. Settle the
caller gate in a finally path or tie it to the drained handler's lifecycle, while retaining observation of the
deliberately held reply.

Suggested bucket: **other**, owned test-fixture contamination. Confidence: **confirmed conditional source mechanism**;
fulfillment rejection in these particular tests is unmeasured. Successful runs are unaffected. No user-owned session
loss or product bug is alleged.

Possible cover: no exact coverage found. Analogous cleanup decisions concern different tests; the rare-glitch filter
does not cover persistent shared fixture state. Proposed drop: none.

Provenance: trailing review through `616d79f45d6a1d817833291429d144a5c686bb4b`, private run
`4fae501c-3686-4822-aa6c-037266637632`, `g061/f1`. No runtime reproduction.
