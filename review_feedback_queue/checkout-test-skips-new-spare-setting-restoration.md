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

## Confirmation-setting instance

Reviewed commit: 3bc9f87d59ba9b9c44ad2987e3aa16bbb365a852

The new disabled-chrome browser test adds the same caller-orchestration gap for the shared session-confirmation
preference. `e2e/tests/chrome.spec.ts:97` writes `skip_session_confirmations=false`; its sequential `finally` awaits
`cleanupSession` before restoration (`114–116`). That helper throws on an unsuccessful response
(`helpers/fleet.ts:556–565`), preventing the restoration request. The worker's test fixture does not reset preferences
between these tests (`helpers/evidence.ts:31–65`). This remains present at review tip
`98bb15850f5f5ba1df6ed66873338261c45ff948`.

Suggested bucket remains **other**. Confidence: **confirmed conditional cleanup omission**. Effective contamination
requires an incoming true preference; no normal runner history supplying it was established. Incoming false or unset
keeps the same effective confirmation mode. Failure frequency was not measured, and no production loss is claimed.

Possible cover: tip ledger `14449–14477,17199–17216` names other test blocks. The newer checkout-specific decision
(`20338–20351`) says to fix observed flakes, with an exception for an obvious trivial fix within about 30 lines. This
new instance has the existing independent-cleanup or nested-finally shape. The decision's scope over this new test
remains a triage question; this extension does not alter its pending execution or reopen a broad cleanup sweep.

Provenance: trailing-review run `47edc805-f43c-4ef2-abb1-0bba27a41c2f/review-98bb1585`, `g034/f1`. Neighbor `g035`
verified that its separate buttons test already uses nested `finally` and does not share this omission.
