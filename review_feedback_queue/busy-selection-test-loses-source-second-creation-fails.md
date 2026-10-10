# Busy-selection test loses its source when the second creation fails

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Failed busy-selection setup leaves acquired sessions outside cleanup.

## Details

F231 — **definite** — `e2e/tests/sidebar.spec.ts:1329` — Busy-selection test loses its source when the second creation
fails

The test acquires session A before its cleanup boundary, then creates B and installs a route. Failure creating B leaves
A running; failure installing the route can leave both. Enter cleanup immediately after acquiring A, track B only once
acquired, and keep route installation inside that boundary so setup errors do not bypass resource ownership.

## Evidence and triage context

- e2e/tests/sidebar.spec.ts:1325–1328 acquires session A.
- e2e/tests/sidebar.spec.ts:1329–1333 acquires B and :1338–1341 installs the route before the try at :1342.
- e2e/tests/sidebar.spec.ts:1367–1371 releases the route and cleans A and B only after execution enters that try.
- e2e/tests/helpers/fleet.ts:416–434 creates and returns the resumable source without a cleanup registry.
- The sidebar file has no terminal-suite reset hook; e2e/tests/helpers/evidence.ts:31–65 does not reclaim sessions.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- The eventual reset in e2e/tests/helpers/terminal-suite.ts:147–186 and shutdown in e2e/start-stack.sh:250–262 limit
  lifetime but do not close this setup failure path. No matching accepted coverage was found.

Caveats:

- The abandoned source is a test fixture, not demonstrated user-owned work.
- Failure during route installation can additionally abandon both sessions.
- Static verification only.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_09_sec:p1:F3`.

- `test_infrastructure_09_sec:p1:F3`: confidence as filed: definite; suggested bucket as filed: other.
