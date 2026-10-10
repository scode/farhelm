# The stop-recovery test can fail the wrong listing request

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The Stop recovery test could inject failure into an earlier listing read.

## Details

F235 — **possible** — `e2e/tests/readers.spec.ts:253` — The stop-recovery test can fail the wrong listing request

Capture numbering starts without proving that initial feed demand has finished. A delayed initial read can consume the
injected failure, while Stop's later successful refetch produces the apparent recovery. That schedule was not reproduced
and no product recovery failure is established. Observe consumption of the initial feed revision and reader idleness
before interception, then identify the captured request that actually belongs to Stop.

## Evidence and triage context

- e2e/tests/readers.spec.ts:244-253 sends a feed notification, checks an already-renderable row and status, then
  installs interception without observing reader idleness.
- e2e/tests/readers.spec.ts:272-290 assumes capture 1 is Stop's refetch and capture 2 is its recovery, but identifies
  neither request by origin.
- crates/farhelm-ui/src/list/view.rs:1606-1627 independently requests mount and feed reads; :1826-1857 performs Stop's
  direct refetch and requests recovery only if that refetch failed.
- e2e/tests/helpers/fleet.ts:742-754 numbers captures when response bodies have been fetched, not by mutation ownership.
- crates/farhelm-ui/src/list/view.rs:1252-1259 gates replies through accepts_listing;
  crates/farhelm-ui/src/ops.rs:274-287 permits an older failure while a newer successful response remains unapplied.
- e2e/tests/readers.spec.ts:244-253 sends a notification, checks visible row state, then installs interception without a
  notification-consumption barrier.
- crates/farhelm-ui/src/list/view.rs:1608-1627 independently requests mount and notification reads.
- e2e/tests/helpers/fleet.ts:737-754 numbers captured replies without identifying the request's originating operation.
- crates/farhelm-ui/src/list/view.rs:1788-1795 and :1826-1857 performs the stop's separate refetch; only its failure
  invokes stop_recovery.
- crates/farhelm-ui/src/ops.rs:282-287 allows an older failure until a newer success commits.
- e2e/tests/readers.spec.ts:274-294 can therefore fail a notification read and restore the list through the
  independently successful stop refetch.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- No exact acceptance, Planned item, BUGS entry, queue item or ledger disposition found. The rare-diagnostic filter does
  not cover false success for the intended recovery proof.

Caveats:

- The scheduling interleaving was not reproduced.
- No claim that the current product recovery implementation is broken.
- Removing stop_recovery was not tested; the possible false-pass path is established by tracing, not mutation execution.
- Requires initial notification demand to reach interception after the initial visible-state assertions. No runtime or
  mutation reproduction performed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_08_cor:p2:F1`,
`test_infrastructure_08_sec:p2:F2`.

- `test_infrastructure_08_cor:p2:F1`: confidence as filed: possible; suggested bucket as filed: other.
- `test_infrastructure_08_sec:p2:F2`: confidence as filed: possible; suggested bucket as filed: other.
