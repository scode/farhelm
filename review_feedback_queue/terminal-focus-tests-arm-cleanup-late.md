# Focus regression tests leave a session outside their cleanup scope

Reviewed commit: e8ad4fa0613a17a4490aa28c4691a61bc0a5a16f

## TLDR

A failure installing the browser focus observer can leave a test's live session behind, contaminating later tests.

## Details

`e2e/tests/terminal-replay-rename.spec.ts:1921–1924,1956–1959` creates a session and then awaits
`installTerminalFocusSpy` before entering the `try` whose `finally` deletes that session. The loop generates two
affected replay cases, with a third case covering selected-tab fallback. The observer helper awaits `page.addInitScript`
(`e2e/tests/helpers/terminal-suite.ts:472–486`), which can reject. That rejection bypasses session cleanup.

The suite hooks at `terminal-suite.ts:196–218` unregister routes after a test and remove tracked scratch directories
after the file. They do not delete these sessions; stack reset occurs before the file, not between its tests. Move the
observer installation inside the cleanup-owning `try`, or install it before creating the session. A focused setup
failure injection can verify that each created session still gets a cleanup attempt.

Suggested bucket: `other`, confined to test infrastructure. Confidence: confirmed static ownership gap; failure
injection was not run. Provenance: separate same-model UI reviewer in the trailing review through
`bf0935f02fc8fbfc26ab54e1b9fb8e7e6bf5c441`; the recording parent checked the tests, helper and hooks.

Possible cover: none found in the SPECs, Planned TODO, BUGS, queue, ledger or filters. Existing
`shortcut-tests-skip-owned-cleanup.md` concerns sequential cleanup failures after cleanup is armed. The ledger's
`download-regression-fixtures-arm-cleanup-too-late.md` has the same ownership shape but explicitly confines its
completed fix to `terminal-files.spec.ts`. It does not cover these new tests. Proposed drop: none.
