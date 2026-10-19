# Shortcut tests skip remaining cleanup after an earlier failure

Reviewed commit: e4207f233dbb6fefcbe52e0ee01fb7ef5ee6331b

Also reviewed: 51f07fa7abfc5dfcd3020a304d8740e5cbe44c16

## TLDR

A failing shortcut test can leave its sessions or templates behind, contaminating later tests.

## Details

The new launcher-shortcut tests delete templates before later owned cleanup
(`e2e/tests/launcher-template-shortcuts.spec.ts:192–195,224–227,299–303`, at the first reviewed commit). A failed
deletion skips the next template or source-session deletion. The template-list tests have the same sequential throwing
cleanup shape (`e2e/tests/template-list-shortcut.spec.ts:152–157,198–202`, at the second reviewed commit): a failed
route removal or session deletion skips the remaining work.

Both files already use `cleanupOwnedSteps` elsewhere. Put each route, session and template cleanup in an independent
step so every owned resource gets an attempt, failures are reported together, and a test-body failure remains primary. A
focused early-cleanup failure injection can establish that later deletions still run.

Suggested bucket: **other**, confined to test isolation. Confidence: **confirmed** from sequential throwing awaits; no
failure injection was run. Provenance: same-model adversarial trailing review of both stated commits. Proposed drop:
none.

Possible cover: the ledger entries `custom-permission-test-skips-template-cleanup.md` and
`number-shortcut-test-skips-session-cleanup.md` record the same mechanism in older, explicitly named tests, with
confined fix scopes. They do not cover these new files. No matching Planned item, known-bug acceptance, existing queue
item or filter was found. `.agents/test-authoring.md` requires checking fixture ownership through failure and
cancellation.
