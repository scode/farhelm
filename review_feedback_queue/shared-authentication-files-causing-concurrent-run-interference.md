# Shared authentication files causing concurrent-run interference

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Overlapping runs could remove each other's authentication state.

## Details

F317 — **possible** — `e2e/tests/helpers/device-auth.ts:8-10` — Shared authentication files causing concurrent-run
interference

The authentication helper and multiple stack cleanup paths share one checkout-local file. An overlapping run can rewrite
or delete state another still needs. Two-agent checkout sharing is excluded, while same-operator overlap remains an
unresolved scope question. Establish that policy and either enforce exclusive ownership of the shared file or supply
per-run authentication paths throughout the helpers and cleanup.

## Evidence and triage context

- device-auth.ts:8-10 and :27-30 uses a single checkout-local storage file; start-stack.sh:194 and :255 removes the same
  file during cleanup; readme-hero/start-stack.sh:89 and :126 does likewise. AGENTS.md:663-669 describes the isolation
  boundary but does not clearly prohibit all same-operator overlap.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_05_sec:p2:C5`.

- `test_infrastructure_05_sec:p2:C5`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.
