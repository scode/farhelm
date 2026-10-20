# Browser tests skip remaining cleanup after an earlier failure

Reviewed commit: e4207f233dbb6fefcbe52e0ee01fb7ef5ee6331b

Also reviewed: 51f07fa7abfc5dfcd3020a304d8740e5cbe44c16

## TLDR

A failing shortcut or naming test can leave sessions, templates or shared checkout settings behind, contaminating later
tests.

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

The trailing review through `bf0935f02fc8fbfc26ab54e1b9fb8e7e6bf5c441` found additional instances in the
managed-checkout naming tests. At `17197671cb4a83d9b59f663eff8ad5e50fcd5f72`,
`e2e/tests/github-checkouts.spec.ts:222–227,305–309` performs route removal, session cleanup, preference restoration,
template deletion and fixture closure sequentially. At `b874f0f09a37b6a19797eb25ebf0f102d6707536`, the new save-template
cases do the same at `:361–363`. An earlier rejected await prevents `fixture.close` from running. That fixture owns
shared checkout-root, post-clone-hook and private Git URL configuration restoration (`:473–503` at the latter commit);
the suite's `afterEach` only restores quick-launch preferences. The fixture's internal cleanup guards cannot help when
its caller never invokes it.

Use independent owned cleanup steps for these new cases too. The consequence remains test isolation, in bucket `other`.
Confidence: confirmed sequential throwing control flow; early-failure injection was not run. Provenance: separate
same-model naming reviewer, with the recording parent checking the cleanup blocks and fixture. The older ledger scopes
above do not include these new tests; no matching Planned item, known-bug acceptance or filter was found.
