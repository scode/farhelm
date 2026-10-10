# Unreachable-operation test does not prove absence of queued work

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The unreachable-host test could accept queuing a refused Stop for later.

## Details

F309 — **possible** — `e2e/tests/terminal-multihost.spec.ts:2597–2603` — Unreachable-operation test does not prove
absence of queued work

Refusal text, row existence, and stale cache all remain compatible with recording a Stop for later delivery.
Reconnection cleanup removes the fixture without first checking that its agent remains running. Current routing does not
queue and no mutation was run. Observe the restored supervisor through an ordered boundary and require no delayed Stop
plus continued fixture liveness before cleanup.

## Evidence and triage context

- e2e/tests/terminal-multihost.spec.ts:2574–2603 expressly tests nothing queued but observes only refusal text, row
  existence and stale=true. At 2478–2479 the group restores the supervisor and immediately cleans the session without
  checking that it remained running. SPEC.md:1028–1031 forbids later delivery.
  crates/farhelm-helm/src/sessions.rs:695–701 and 2040–2042 currently refuse without queuing; this proves current
  behavior, not the oracle's ability to reject the stated regression. No matching cover was found.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_12_cor:p1:C1`.

- `test_infrastructure_12_cor:p1:C1`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.
