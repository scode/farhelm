# History fixture’s policy restoration can skip its remaining cleanup

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

History-test restoration failure skips cleanup of its known session.

## Details

F261 — **definite** — `e2e/tests/sidebar.spec.ts:5056` — History fixture’s policy restoration can skip its remaining
cleanup

The finally block awaits policy restoration before closing the second context and deleting its registered session. A
restoration or verification error skips both later operations, leaving server-side session resources behind. Use nested
finally blocks or cleanup aggregation so each independent cleanup is attempted despite earlier failures, while
preserving the original and cleanup errors.

## Evidence and triage context

- e2e/tests/sidebar.spec.ts:5001-5004 creates the second context; 5018-5025 admits a real session and saves its ID.
- e2e/tests/sidebar.spec.ts:5055-5058: a rejection at policy restoration exits finally before either second.close or
  cleanupSession is called.
- e2e/tests/helpers/fleet.ts:321-332: restoration can commit false and then fail on its independent readback, so failure
  does not establish that session cleanup is unavailable.
- e2e/tests/helpers/fleet.ts:505-515: cleanupSession uses separate stop and delete requests.
- e2e/tests/helpers/evidence.ts:68-75: newObservedContext returns a caller-owned context and does not register session
  cleanup.
- e2e/start-stack.sh:242-265: eventual stack teardown is a separate outer cleanup boundary; it does not perform this
  test's deletion before later tests.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- {"basis": "TODO.md:255-261, Shared-checkout browser timeout and fixture cleanup", "comparison": "This describes
  session-cleanup failure preventing Git URL-map restoration in github-checkouts.spec.ts. Here policy restoration
  prevents session deletion in sidebar.spec.ts. Triggering operation, retained resource, and call-site scope differ; the
  TODO item is also outside Planned."}

Caveats:

- No runtime reproduction was performed.
- Skipping explicit context.close is definite, but a browser-worker shutdown can subsequently reclaim that context. An
  indefinite browser-context leak is not established.
- The retained session belongs to the private test stack and can survive browser teardown until another deletion or
  stack cleanup.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_10_cor:p3:F3`.

- `test_infrastructure_10_cor:p3:F3`: confidence as filed: definite; suggested bucket as filed: other.
