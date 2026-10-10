# The multi-session test does not hold its DELETE request

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The multi-session test's DELETE gate does not match the real request.

## Details

F245 — **definite** — `e2e/tests/terminal.spec.ts:1731` — The multi-session test does not hold its DELETE request

The route glob ends at the session identifier, but deleting the stopped session adds a query string. The real deletion
bypasses the intended hold and can remove the row before the supposedly held-state assertion. Match the URL pathname
independently of the query and observe handler arrival before checking the row.

## Evidence and triage context

- e2e/tests/terminal.spec.ts:1713-1719 stops B and observes its ended status.
- e2e/tests/terminal.spec.ts:1731-1737 registers the queryless route; :1743 requires the row to remain present before
  release.
- crates/farhelm-ui/src/list/view.rs:2011-2042 selects DeleteGuard::NothingAlive for an ended, tabless session.
- crates/farhelm-ui/src/api.rs:2636-2641 adds ?only_if_nothing_alive=true.
- e2e/tests/terminal.spec.ts:3666-3670 uses a pathname predicate at another fixture specifically to avoid this mismatch.
- e2e/tests/terminal.spec.ts:1764-1767 already releases this gate unconditionally.
- e2e/tests/terminal.spec.ts:1731 installs the route; :1743 asserts B’s row remains.
- crates/farhelm-ui/src/list/view.rs:2011–2042 selects the stopped-session guard; crates/farhelm-ui/src/api.rs:2636–2642
  adds ?only_if_nothing_alive=true.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- TRIAGE_OUTCOMES.md:2193-2227 records the product liveness safeguard and its dedicated browser regression. It does not
  cover stale route matching in this separate test.
- Same finding as test_infrastructure_14_sec:p1:F1.

Caveats:

- The mismatch is definite; whether the row assertion fails depends on deletion timing.
- No runtime reproduction was performed.
- No runtime reproduction. Distinct from the exited-session test; no teardown hang claimed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_14_sec:p1:F1`,
`test_infrastructure_14_cor:p2:F1`.

- `test_infrastructure_14_sec:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
- `test_infrastructure_14_cor:p2:F1`: confidence as filed: definite; suggested bucket as filed: other.
