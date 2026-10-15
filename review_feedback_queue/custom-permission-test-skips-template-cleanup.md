# One failed template cleanup skips the remaining custom-permission fixtures

Reviewed commit: a3ff0b6b617cad499ab4b129ef4282861b70436b

## TLDR

A failed browser-test cleanup can leave unwanted templates in the shared test catalog and affect later tests.

## Details

Suggested bucket: `other`. Confidence: confirmed by static failure-path tracing; not reproduced. Source: separate
adversarial trailing-review agent, permissions scope. Proposed drop: none.

`e2e/tests/custom-permissions.spec.ts:228–232` deletes four templates in one throwing loop inside `finally`. If the
first DELETE rejects or its status assertion fails, none of the later DELETEs is attempted. The suite hooks in
`e2e/tests/helpers/terminal-suite.ts:147–215` reset sessions and routes, not templates. The fixtures therefore remain in
the shared catalog. The mechanism persists at `507dbcbbcfa00173ca7e4d24c79f3990025963b6`.

Attempt every owned deletion independently, then report collected failures. A focused failure injection can establish
that later DELETEs still occur after the first fails. This affects test-owned catalog entries; no shipped-product data
loss was established.

Possible cover: `template-regression-fixtures-arm-cleanup-too-late.md` covers different tests that fail before entering
their cleanup guards. It does not cover this failure inside an already-owned cleanup loop. No exact Planned, accepted
bug, ledger or filter coverage was found.

## Permission-switch fixtures have the same cleanup failure

Reviewed commit: d17d26750c56a438618920796cfd6ff120e28dfb

Suggested bucket: `other`. Confidence: confirmed by static failure-path tracing; failure injection was not run. Source:
separate adversarial trailing-review agent, permission safeguards scope. Proposed drop: none; this adds affected
fixtures to the existing finding.

`e2e/tests/permission-switch.spec.ts:147–150` deletes two owned templates in one throwing loop. Both can have been
created at `:110–115`; a rejected DELETE or failed status assertion skips the other deletion. The shared stack reset at
`helpers/terminal-suite.ts:147–187` resets sessions, not templates. Attempt both deletions independently and preserve
their failures. This persists at `75af240c09c7ecd5b81a0fcba9e03f204b0be0a7`.

Possible cover: the original section covers the same mechanism in a different test.
`template-regression-fixtures-arm-cleanup-too-late.md` concerns failures before ownership, rather than within cleanup.
No shipped-product regression was established. All five assigned safeguard commits were read completely; surrounding
code and ledger sections were inspected selectively.
