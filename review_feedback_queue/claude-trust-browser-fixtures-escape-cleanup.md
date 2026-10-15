# Claude trust browser fixtures can escape cleanup

Reviewed commit: 0ad806334217ba8dde4e7f7dc6ecc7480d27cb9a

## TLDR

A failed trust-hint browser test can leave agents running or shared trust preferences changed for later tests.

## Details

Suggested bucket: `other`. Confidence: confirmed by static failure-path tracing; failure injection was not run. Source:
separate adversarial trailing-review agent, Claude trust scope, with coordinator verification of the cleanup body.
Proposed drop: none.

`e2e/tests/claude-trust.spec.ts:11–12` creates the first session before creating its neighbor; a failure of the second
create leaves the first without cleanup ownership. Creating the observer at `:35` is another fallible operation before
`try`. Inside `finally` at `:131–137`, closing that page, restoring preferences, resetting trust, and deleting both
sessions are sequential. Any earlier failure skips the remaining cleanup. The evidence fixture does not sweep these
sessions or shared preferences.

Acquire cleanup ownership immediately after each resource is created. Attempt every restoration and owned deletion
independently, preserving the failures. Inject failure during the second create and an early cleanup step, then verify
that existing sessions and preference restorations are still handled. No shipped-product loss was established.

Possible cover: the queue has several cleanup-ownership findings, but they name other tests and resources; none covers
this new trust fixture. No exact Planned item, specification acceptance, BUGS entry, ledger decision, or filter was
found.

The complete introducing patch was read and the coordinator separately inspected the complete browser test. No runtime
browser test was run.
