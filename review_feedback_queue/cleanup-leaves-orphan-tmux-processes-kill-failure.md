# Cleanup leaves orphan tmux processes after kill failure or budget exhaustion

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Failed or skipped tmux shutdown could leave an orphan without its socket path.

## Details

F310 — **possible** — `crates/farhelm-teststate/src/lib.rs:471` — Cleanup leaves orphan tmux processes after kill
failure or budget exhaustion

The stale-state sweep removes directories even when kill attempts fail or the shared budget expires before remaining
servers are tried. Removing their socket paths prevents later pathname-based retries, so test-owned daemons can survive
cleanup. This is possible correctness, not established unrelated-work destruction. Return explicit per-server shutdown
outcomes and retain retryable state when absence has not been confirmed.

## Evidence and triage context

- crates/farhelm-teststate/src/lib.rs:461-471 skips remaining kills once the shared budget expires and still removes
  their directories. :493-527 does not return a success verdict to reap. Removing the socket path prevents a later
  pathname sweep from retrying that server. The acceptance claim is only in comments at :431-448 and :113-118; no
  matching SPEC, Planned, BUGS, queue or ledger disposition was found. No unrelated-work destruction is established.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_01_sec:p2:C7`.

- `test_infrastructure_01_sec:p2:C7`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.
