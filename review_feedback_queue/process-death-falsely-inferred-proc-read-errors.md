# Process death falsely inferred from /proc read errors

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A later process-read error could be mistaken for confirmed death.

## Details

F313 — **possible** — `e2e/tests/terminal-tabs.spec.ts:90–125` — Process death falsely inferred from /proc read errors

Initial identity capture proves readability only at setup. Later exceptions all become not-alive, so a non-absence read
failure can satisfy the kill assertion while the same process remains running. No such error was reproduced, and
wrong-process signaling is not established. Distinguish confirmed disappearance or identity replacement from observation
errors and surface inconclusive inspection instead of accepting it as death.

## Evidence and triage context

- terminal-tabs.spec.ts:93–95 converts every read exception to undefined; :121–125 converts undefined to not alive;
  :306–310 proves only initial readability; :345–350 subsequently accepts not alive as proof of the kill. No specific
  later read-error occurrence was reproduced. review_feedback_queue/birth-oracle.md:14–23 concerns a different probe,
  trigger and checkout-ownership scope, so it is not coverage.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_14_cor:p1:C2`.

- `test_infrastructure_14_cor:p1:C2`: confidence as filed: possible; suggested bucket as filed: not separately tagged in
  candidate list.
