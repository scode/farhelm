# Shutdown-race test does not establish the intended interleaving

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The shutdown-race test does not hold reconciliation at the database boundary.

## Details

F171 — **definite** — `crates/farhelm-helm/src/manager.rs:10302` — Shutdown-race test does not establish the intended
interleaving

Reconciliation can finish against the unchanged registry before shutdown, starting no new worker. Shutdown then empties
the existing map and all assertions pass, even if the production shutdown check has moved to the wrong side of the
database await. Yielding does not establish the intended overlap. Acknowledge and hold reconciliation at that boundary,
shut down, then release it and inspect workers and connection attempts.

## Evidence and triage context

- crates/farhelm-helm/src/manager.rs:10269–10281: the test claims shutdown interrupts reconciliation's registry await.
- crates/farhelm-helm/src/manager.rs:10300–10307: it spawns reconciliation, yields once, then shuts down.
- crates/farhelm-helm/src/manager.rs:10309–10321: assertions check the empty map and unchanged dial count.
- crates/farhelm-helm/src/manager.rs:1470–1478: production currently checks shutdown after the awaited read.
- crates/farhelm-helm/src/manager.rs:10279-10281: the test documentation incorrectly calls yield_now a barrier reaching
  the store read.
- crates/farhelm-helm/src/manager.rs:10300-10307: spawns reconciliation, yields once, then shuts down without a
  readiness or release gate.
- crates/farhelm-helm/src/manager.rs:1468-1478: the protected boundary is between the awaited registry read and the
  actor-map lock and shutdown check.
- crates/farhelm-helm/src/manager.rs:10309-10321: assertions establish terminal shutdown behavior but do not establish
  that the contested interleaving occurred.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- {"matched": null, "possible": null}

Caveats:

- Concrete test-oracle defect; no current production shutdown regression established.
- No mutation experiment.
- No current production shutdown regression is established.
- No mutation experiment was run.
- Static verification only; no mutation test.
- Removing every shutdown check would still be caught by the later explicit reconciliation. The defect is missing proof
  of the concurrent-read race, not absence of all shutdown coverage.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_helm_connections_sec_04:p1:F1`, `hc_general:p1:F1`.

- `gap_helm_connections_sec_04:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
- `hc_general:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
