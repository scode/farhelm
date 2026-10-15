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
