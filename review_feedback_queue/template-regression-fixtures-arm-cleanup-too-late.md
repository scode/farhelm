# New template regressions arm cleanup after fallible setup

Reviewed commit: e64574a9158d1aec5ef07461221835e9b6ea950e

## TLDR

A failed browser-test setup can leave unwanted templates in the shared test catalog.

## Details

Suggested bucket: `other`. Confidence: confirmed by static failure-path tracing; not reproduced. Source: dedicated
`gpt-6.1-sol` high adversarial review. Proposed drop: none.

e2e/tests/templates.spec.ts:83–98 saves its template before entering try/finally. The subsequent GET, response/header
assertions, or route registration can fail without reaching deletion at :119. e2e/tests/quick-switcher.spec.ts:34–47 has
the same gap, with deletion only at :65. Trigger: successful template storage followed by a setup failure. Consequence:
the fixture remains in the shared helm catalog for later tests; no production user data or processes are lost.
helpers/evidence.ts:31–65 only collects evidence, and terminal-suite.ts:147–186 resets sessions, not templates.
Establish cleanup ownership before attempting the PUT and keep all subsequent setup inside that guard.

Possible cover: review_feedback_queue/header-regression-fixtures-escape-cleanup.md describes analogous session fixtures
in another test file; its trigger and resource scope do not cover these template fixtures. No exact Planned, BUGS,
ledger, queue, or filter coverage found.

Current-main status: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: templates.spec.ts:84–99,119–120 and
quick-switcher.spec.ts:34–47,63–65 retain the gap. No fixing commit found.

Review coverage: the full commit diff and relevant failure paths were read. No runtime reproduction was run; unchanged
surrounding code was inspected only where relevant.
