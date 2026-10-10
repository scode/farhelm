# Resize teardown accepts a refused deletion as success

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Resize-test teardown reports success after a refused deletion.

## Details

F244 — **definite** — `e2e/tests/sidebar-resize.spec.ts:172` — Resize teardown accepts a refused deletion as success

The cleanup request can resolve with an HTTP refusal, which the test ignores. Teardown then succeeds while the live
fixture session remains for later tests. Direct deletion is valid; treating every response as cleanup success is the
defect. Accept successful removal or an already-absent session, and surface all other statuses.

## Evidence and triage context

- e2e/tests/sidebar-resize.spec.ts:146-150 creates a real sleep 300 session.
- e2e/tests/sidebar-resize.spec.ts:171-173 awaits DELETE but never checks its response.
- e2e/tests/helpers/fleet.ts:505-515 explicitly rejects unsuccessful cleanup responses other than 404.
- e2e/playwright.config.ts:102-112 supplies request defaults without enabling failOnStatusCode.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- SPEC.md:821-826 permits direct Delete to terminate live work. It does not excuse reporting successful fixture cleanup
  after a refused request. FILTER.md:41 excludes success reported for failure.

Caveats:

- Requires a non-success deletion response.
- The established consequence is test-fixture survival, not user-data loss.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_09_cor:p1:F5`.

- `test_infrastructure_09_cor:p1:F5`: confidence as filed: definite; suggested bucket as filed: other.
