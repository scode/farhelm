# The exited-session test also bypasses its DELETE gate

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The exited-session test separately bypasses its deletion hold.

## Details

F246 — **definite** — `e2e/tests/terminal.spec.ts:2119` — The exited-session test also bypasses its DELETE gate

This fixture also matches a full URL ending at the identifier while the actual guarded DELETE carries a query. Correct
deletion can therefore remove the session before its held-row assertion, replacing a controlled premise with a race.
Match pathname, wait for handler arrival, and release the gate in finally before route-draining teardown.

## Evidence and triage context

- e2e/tests/terminal.spec.ts:2102-2106 creates a session running true; :2131-2133 observes exited status.
- e2e/tests/terminal.spec.ts:2119-2125 registers a glob ending at the ID and waits on deleteHeld only if it matches.
- crates/farhelm-ui/src/list/view.rs:2011-2042 and crates/farhelm-ui/src/api.rs:2636-2641 produce the query-bearing
  guarded DELETE.
- e2e/tests/terminal.spec.ts:2141-2145 assumes the row remains until release.
- e2e/tests/terminal.spec.ts:2146-2147 omits releaseDelete from finally; e2e/tests/helpers/terminal-suite.ts:198-205
  waits for active route handlers during teardown.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- TRIAGE_OUTCOMES.md:2193-2227 covers the product safeguard, not this independently editable acceptance fixture.

Caveats:

- The current normal request bypasses the handler, so the missing finally release is a repair requirement rather than
  proof of a current normal-path hang.
- No runtime reproduction was performed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_14_sec:p1:F2`.

- `test_infrastructure_14_sec:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
