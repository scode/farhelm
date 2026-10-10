# A failed clone setup leaves the hosts route permanently held

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Early clone-test failure leaves a route gate unreleased.

## Details

F238 — **definite** — `e2e/tests/terminal-multihost.spec.ts:2397` — A failed clone setup leaves the hosts route
permanently held

Once a hosts request enters the held handler, an assertion failure before the successful-path release leaves its promise
unresolved. Session cleanup cannot settle it, and the installed teardown waits for active handlers, adding another
timeout and failure noise. Release the held hosts gate unconditionally at the start of finally, before any fallible
cleanup.

## Evidence and triage context

- e2e/tests/terminal-multihost.spec.ts:79 installs the terminal suite hooks.
- e2e/tests/terminal-multihost.spec.ts:2334–2342 creates the gate and awaits it for hosts GETs.
- e2e/tests/terminal-multihost.spec.ts:2348–2365 has navigation and assertions before the sole release at 2368.
- e2e/tests/terminal-multihost.spec.ts:2397–2400 cleans sessions without releasing the gate.
- e2e/tests/helpers/terminal-suite.ts:198–205 calls unrouteAll with behavior wait.
- e2e/tests/terminal-multihost.spec.ts:2334–2342 creates and awaits heldHosts.
- e2e/tests/terminal-multihost.spec.ts:2368 contains its sole release.
- e2e/tests/terminal-multihost.spec.ts:2397–2400 omits release from finally.
- e2e/tests/terminal-multihost.spec.ts:79 installs e2e/tests/helpers/terminal-suite.ts:198–205, whose teardown waits for
  active routes.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- TODO.md:255–261 concerns Git URL-map restoration in github-checkouts.spec.ts, not unresolved route handlers.
- TODO.md:255–261 describes another fixture's Git configuration restoration, not a hosts-route promise.

Caveats:

- The teardown consequence requires an active intercepted hosts GET.
- Playwright bounds the resulting wait; this is an additional timeout, not an established infinite whole-run hang.
- Duplicate input retained separately.
- Requires a hosts GET to have entered the handler before failure.
- No runtime reproduction.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_12_cor:p1:F1`,
`test_infrastructure_12_sec:p1:F2`.

- `test_infrastructure_12_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
- `test_infrastructure_12_sec:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
