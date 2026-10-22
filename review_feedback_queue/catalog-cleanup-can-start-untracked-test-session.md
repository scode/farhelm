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

### Pending key mint in two further cleanup paths

Additional reviewed commit: `c06589136a40e0a5223bb97f3fad7b58188268d7`. In `e2e/tests/launcher-enter.spec.ts:1038`,
Enter starts a separate scope-owned submission; it does not join it (`:442–450`;
`crates/farhelm-ui/src/list/create_form.rs:5072,5460–5462,5594–5597`). The refusal assertion at test line 107 can time
out while submission awaits the browser key mint (`create_form.rs:5113`; `api.rs:2926–2929,2981–2989`). Finally removes
routes at `launcher-enter.spec.ts:1072`, then awaits template deletion at `1074` with the form mounted. Mint completion
during that wait can pass unchanged-draft checks and POST after the refusal observer was removed
(`create_form.rs:5155–5224,5354–5365`; `api.rs:2432–2437`). There is no session cleanup in this block, and no entered
POST handler yet for `unrouteAll(wait)` to drain.

In `terminal-create-idempotency.spec.ts:593–594,638–639`, the same pre-POST wait can outlive the assertion and the
single cleanup listing (`helpers/terminal-suite.ts:334–340`). A mint completing after that snapshot but before disposal
can admit a session outside cleanup. The later awaited cleanup provides a live-form interval; this differs from merely
assuming route removal always races page disposal. Eventual unmount cancels a still-pending task, so admission must win
before disposal. Ordinary Cancel and Escape are busy-refused (`create_form.rs:5588–5591,5707–5719`).

Confidence: **confirmed conditional source path**, with adverse scheduling and successful allocation unmeasured.
Suggested bucket: other. These stacks use fake Codex (`e2e/start-stack.sh:146–159,211–250`), including in real-Claude
mode; this is not evidence of vendor turns or user-owned work loss. Existing cleanup acceptance at
`TRIAGE_OUTCOMES.md:17199–17216` covers late already-entered replies in eight named blocks, not these pre-POST tasks. No
exact other coverage found; proposed drop: none. Provenance: trailing review through
`616d79f45d6a1d817833291429d144a5c686bb4b`, run `4fae501c-3686-4822-aa6c-037266637632`, `g037/f1`. No runtime
reproduction.
