# Checkout test can skip restoring the new spare setting

Reviewed commit: 5540207de26f291e29919c8611d83a859cb8fc22

## TLDR

A failed checkout browser test can leave prepared checkouts disabled for later tests on its private stack. An earlier
cleanup failure can prevent the fixture's restoration from running at all. The teardown structure predates this change;
the change adds another shared setting it can leave behind.

## Details

Suggested bucket: **other**, for private test-stack contamination. Confidence: **confirmed conditional source
mechanism**. Cleanup failure was not reproduced, and its frequency is unmeasured. Historical coverage of this new
consequence is ambiguous.

`e2e/tests/github-checkouts.spec.ts:450–452,508–510` saves the old prepared-checkout setting and disables it. Successful
setup leaves restoration to `fixture.close()`. The quick-launch caller's `finally` block awaits route cleanup, session
cleanup, a preference update and template deletion before calling `fixture.close()` (`143–149`). A failure in any
earlier step skips that call. For example, `patchPreferences` throws on an unsuccessful response
(`e2e/tests/helpers/fleet.ts:1080–1100`), so failure at caller line `147` prevents closure at `149`. Session cleanup can
also throw (`helpers/fleet.ts:556–567`). Other sequential callers share the omission.

The fixture's own cleanup steps are independent (`github-checkouts.spec.ts:543–566`; `helpers/cleanup.ts:6–18`), so this
is a caller-orchestration gap. It differs from an outstanding final settings save overtaking restoration: no restoration
request is sent here.

Possible cover: at pinned tip `2ea2826caa0eec867c4e2bddc01badbac7bbe6c7`, `TRIAGE_OUTCOMES.md:17199–17216`,
`shortcut-tests-skip-owned-cleanup.md`, records the same class and explicitly limits its fix to named blocks, leaving
roughly eleven unnamed blocks outside implementation scope. It acknowledges skipped checkout-setting restoration, but
predates this additional switch mutation. This record preserves only the new setting consequence and that ambiguity; it
does not reopen the approved fix's scope. The named trash/agent cleanup decision at `14417–14447` concerns other
callers. No exact Planned, BUGS, current queue or FILTER coverage was found. Proposed drop: none. Resolve that coverage
question before commissioning a broader cleanup sweep.

Provenance: trailing-review run `30f46c42-a021-475f-96a8-eac06930471b`, `g018/f2`, found in discovery pass 2. Auditor
`g045` independently noticed the sequential caller but treated it as the neighboring historical concern; the new shared
switch is the retained remainder.
