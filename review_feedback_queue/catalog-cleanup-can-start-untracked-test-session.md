# Catalog cleanup can start a session after observation ends

Reviewed commit: 67d38483612e37c305720deecfb9357b03ff4ebc

## TLDR

A failed browser test may leave an extra running test session behind, affecting later tests on its stack. The required
browser/Playwright scheduling has not been reproduced.

## Details

`e2e/tests/template-list-shortcut.spec.ts:134–153` holds the launch catalog while a separate route records created IDs.
Cleanup at `157–161` releases the catalog, drains existing route handlers, removes routes, then deletes captured IDs.
That covers a late reply to an existing intercepted create, not a create initiated after observation ends. At this
revision, `crates/farhelm-ui/src/list/create_form.rs:4544–4596` waits for the catalog before applying the template; the
separate auto-launch effect (`4782–4861`) later queues submission through `1276–1282`. Cleanup neither cancels the
opening nor establishes application settlement before removing observation. Quiesce the launcher or keep observing
through that boundary while preserving independent release attempts and the deliberately held catalog fixture.

Suggested bucket: **other**, confined to owned test fixtures. Confidence: **likely**; application phases are
established, but successful POST after route removal before page/template teardown is unverified. Proposed drop:
inherited ordering may limit attribution; the parent already releases/unroutes/cleans at `132–155`. Pre-existence is not
acceptance.

Possible cover: final `TRIAGE_OUTCOMES.md:17199–17216` limits the repair to eight cleanup blocks and late existing
replies, not subsequently initiated creates. `SPEC_impl.md:1142–1155` describes auto-submission; `4654–4667` accepts
tooling threat/concurrency assumptions, not this intra-test race. No exact Planned, BUGS, queue or filter match was
found. Provenance: `test-cleanup-tests-1/f1`, independently corroborated by `test-cleanup-contracts-1/f1`, trailing
review through `fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`. No runtime reproduction was performed.

Original finding IDs: `test-cleanup-contracts-1/f1`, `test-cleanup-tests-1/f1`.
