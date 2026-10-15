# A failed pin test can leave its sessions running

Reviewed commit: b662bd98587768017fda15f9e863731a42bce20f

## TLDR

A failed browser test can leave its agents and pinned rows in the shared test stack, affecting later tests.

## Details

Suggested bucket: `other`. Confidence: confirmed by static failure-path tracing; failure injection was not run. Source:
separate adversarial trailing-review agent, pins and live permission display scope. Proposed drop: none.

`e2e/tests/pins.spec.ts:61–64` awaits preference restoration and closing its observer page before deleting either owned
session. Either rejection skips both deletions. Its deletion loop also stops at the first failure, skipping the other
session. `e2e/tests/helpers/fleet.ts:520–530` shows that cleanup can throw on transport errors or refusals. The imported
evidence fixture in `helpers/evidence.ts:20–65` records browser evidence without sweeping sessions; this test does not
install the terminal suite's broader reset hooks.

Attempt preference restoration, observer closure, and every owned session deletion independently, then report collected
failures. Inject an early cleanup failure and verify that both session deletions are still attempted. No shipped-product
data loss was established.

Possible cover: `appearance-test-restoration-skips-session-cleanup.md`, `header-regression-fixtures-escape-cleanup.md`,
and `listing-regression-fixtures-escape-cleanup.md` concern other fixtures. None covers the new pin test. No matching
Planned item, accepted bug, ledger decision, or filter was found.

The complete pin and live-permission commits were read. No changed scope went unread; surrounding code and the ledger
were inspected selectively. No runtime tests or checkout mutations were performed by the reviewer.
