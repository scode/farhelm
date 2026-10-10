# Failed add-host assertions leave an extra registered host behind

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A failed add-host test leaves a registration that later resets do not remove.

## Details

F258 — **definite** — `e2e/tests/terminal-multihost.spec.ts:3770-3771` — Failed add-host assertions leave an extra
registered host behind

Host registration succeeds before several assertions and before the cleanup identifier is stored. Failure in that
interval skips removal, while shared-stack resets leave extra hosts registered, causing later baseline failures. Recover
the uniquely named host in finally when the identifier was not captured, and require successful deletion rather than
assuming cleanup occurred.

## Evidence and triage context

- e2e/tests/terminal-multihost.spec.ts:3754 submits registration; :3756-3760 can throw before :3767 assigns added.
- e2e/tests/terminal-multihost.spec.ts:3771 removes a host only when added is populated.
- e2e/tests/helpers/terminal-suite.ts:159-186 resets sessions, not host registrations.
- e2e/tests/terminal-multihost.spec.ts:547-574 restores the designated remote; it does not sweep this unique
  destination.
- e2e/playwright.config.ts:85-100 shares one stack across projects; terminal-multihost.spec.ts:684 requires exactly two
  hosts.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- TODO.md:255-261 addresses Git fixture URL mappings, not leaked host registrations, and is outside Planned.
  TRIAGE_OUTCOMES.md:4378-4391 addresses a browser operation lock after re-login, not harness fleet contamination.

Caveats:

- Requires an assertion or lookup failure after registration succeeds. No runtime reproduction performed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_12_cor:p2:F1`.

- `test_infrastructure_12_cor:p2:F1`: confidence as filed: definite; suggested bucket as filed: other.
