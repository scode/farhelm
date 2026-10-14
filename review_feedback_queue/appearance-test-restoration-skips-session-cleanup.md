# Appearance-test restoration failure skips session cleanup

Reviewed commit: bb10ca3de869dc3c24cfa4ce89096c28fd0bd0ea

## TLDR

A failed browser test can leave its session and agent running in the shared test stack.

## Details

Suggested bucket: `other`. Confidence: confirmed by static failure-path tracing; not reproduced. Source: dedicated
`gpt-6.1-sol` high adversarial review. Proposed drop: none.

At e2e/tests/terminal-multihost.spec.ts:1339–1343, cleanup runs sequentially: close the observer context, restore
appearance, assert restoration succeeded, then stop/delete the session. A context-close exception, transport error
during restoration, or non-success restoration response skips cleanupSession entirely, even when session cleanup could
succeed. The session was acquired at :1273. helpers/terminal-suite.ts:196–215 provides route draining but no after-test
session sweep; its :147–187 reset runs before the next file/worker, rather than guaranteeing this teardown. Attempt
context closure, appearance restoration, and session cleanup independently, preserving all failures. Scope is owned test
fixtures, not loss of user-owned work.

Possible cover: TRIAGE_OUTCOMES.md:12426–12446 covers analogous sidebar history/keyboard fixtures;
review_feedback_queue/header-regression-fixtures-escape-cleanup.md covers different header fixtures. Neither matches
this test's scope. No matching appearance finding found in the queue or ledger.

Status at frozen endpoint: Persists at pinned main 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703,
e2e/tests/terminal-multihost.spec.ts:1429–1433. The same sequential cleanup remains; no fixing commit found.

Review coverage: full commit diff read: true. No commit diff content remains unread. Surrounding files were inspected
only in the listed ranges. Unrelated TRIAGE_OUTCOMES.md entry bodies were not read exhaustively; coverage checking used
its complete headings plus targeted searches and relevant entries. An initial broad search was truncated and was
replaced by bounded targeted reads; its missing output was not treated as reviewed. No independent highest-drop review
was performed: no covered, filtered, or resolved highest finding is proposed for dropping.

Runtime evidence: None. Static review only; no checkout files created, modified, or deleted. Applied the supplied
test-authoring checklist to changed tests and fixtures.
