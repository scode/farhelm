# Failed session cleanup skips the new test’s configuration restoration

Reviewed commit: 99fe4cc89be27c4788ca082f7b1fb3e3bcd3ea4c

## TLDR

A cleanup failure in the new browser test can contaminate later tests with its checkout settings, Git rewrite and
retained archives.

## Details

Suggested bucket: `other`. Confidence: confirmed by static failure-path tracing; not reproduced. Source: dedicated
`gpt-6.1-sol` high adversarial review. Proposed drop: none.

e2e/tests/github-checkouts.spec.ts:1215–1217 awaits cleanupSession before fixture.close, so a thrown cleanup error skips
closure entirely. cleanupSession throws on transport errors or non-404 refusals (e2e/tests/helpers/fleet.ts:520–530).
The fixture changes the shared private Git configuration and checkout root/post-clone command at
github-checkouts.spec.ts:106–109; restoration and scratch removal belong to close at 114–132. Trigger: cleanup fails
after either successful execution or a setup/assertion failure. Consequence: subsequent tests inherit fixture state and
possibly leftover archives or sessions. Scope: the owned browser-test stack, not user installations. Put fixture.close
in an independent finally, preserving cleanup errors; the preceding trash test already uses that structure at 1169–1173.

Possible cover: review_feedback_queue/confirmation-test-optout-survives-cleanup-failure.md and TRIAGE_OUTCOMES.md
heading history-fixtures-policy-restoration-skip-remaining-cleanup.md concern analogous failures in different fixtures
and different shared state; neither fully covers this added test.

Current-main status: Persists at pinned main 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703,
e2e/tests/github-checkouts.spec.ts:1308–1310. No fixing commit found.

Review coverage: the full commit diff and relevant failure paths were read. No runtime reproduction was run; unchanged
surrounding code was inspected only where relevant.

## Additional affected scope

Reviewed commit: c8de519d6351af429a9c2ff519bc54774a97b8ed

Confidence: confirmed by inspection; failure paths not executed. Suggested bucket: `other`. Source: dedicated
`gpt-6.1-sol` high adversarial review. This extends the same finding rather than adding a duplicate.

e2e/tests/github-checkouts.spec.ts:141–144 creates the checkout fixture, which changes shared Git configuration and
checkout settings, before context.newPage and before entering try. A newPage failure skips fixture.close entirely. The
new finally at 208–213 also chains cleanup sequentially: driver.close or cleanupSession failure skips both setting
resets and fixture.close; a reset failure likewise skips subsequent cleanup. cleanupSession explicitly throws on non-404
failures (e2e/tests/helpers/fleet.ts:520–529). Scope is the owned browser test stack, not production user state. Arm
ownership before setup can fail and attempt each independent cleanup even when another fails.

Possible cover: TRIAGE_OUTCOMES.md headings history-fixtures-policy-restoration-skip-remaining-cleanup.md and
keyboard-launch-fixtures-policy-restoration-skip-session-deletion.md concern other tests. The current queue’s
confirmation-test-optout-survives-cleanup-failure.md concerns a different fixture and setting.

Current-main status: Persists at pinned main: e2e/tests/github-checkouts.spec.ts:231–234 and 298–303. No fixing commit
identified.
