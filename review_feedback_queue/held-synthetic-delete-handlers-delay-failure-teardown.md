# Held synthetic DELETE handlers can delay failure teardown

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Failed assertions could leave synthetic DELETE handlers blocking teardown.

## Details

F318 — **possible** — `e2e/tests/terminal.spec.ts:3677` — Held synthetic DELETE handlers can delay failure teardown

Both held-delete fixtures release their promises only on the successful path. An assertion failure after a handler
enters leaves it unresolved, while the actual afterEach waits for outstanding route handlers until timeout. No user-work
loss is claimed. Release each gate unconditionally in finally before fallible cleanup and route draining, so the
original failure does not strand teardown.

## Evidence and triage context

- terminal.spec.ts:3677 and :3756 await promises released only at :3703 and :3789, with no finally release.
  helpers/terminal-suite.ts:198-205 waits for outstanding route handlers in afterEach. FILTER.md:24-34 requires both a
  rare trigger and a wholly qualifying consequence; that full match is not established merely by calling this a
  secondary diagnostic.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_15_sec:p1:C5`.

- `test_infrastructure_15_sec:p1:C5`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.
