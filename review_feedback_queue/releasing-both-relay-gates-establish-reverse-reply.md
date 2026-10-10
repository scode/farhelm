# Releasing both relay gates does not establish reverse reply order

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The relay test can pass without replies arriving in reverse order.

## Details

F130 — **definite** — `crates/farhelm/tests/e2e/agent_relay.rs:674` — Releasing both relay gates does not establish
reverse reply order

Releasing both reply gates allows the answer tasks to run in request order. Even reversed peer send order would not by
itself establish upstream arrival order. An incorrect arrival-order response matcher can consequently pass the claimed
reverse-order regression. Preserve observed request order and receive the last request's correctly correlated reply
before releasing the first request.

## Evidence and triage context

- agent_relay.rs:95–97 releases a semaphore permit; :113–121 waits before constructing the answer. :650–664 collects
  arrivals and sorts away their order. :674–675 releases both handlers before either answer is observed at :677–678.
- agent_relay.rs:95–97 only adds semaphore permits; :650–664 discards handler arrival order; :674–678 releases both
  handlers before observing a response.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- SPEC_impl.md:1284–1286 establishes per-connection request IDs and round-trip mapping.
  .agents/test-authoring.md:6–9,16–17 requires the fixture premise and distinguishing observable. No exact existing
  coverage found.
- SPEC_impl.md:1284–1286 requires round-trip request-ID mapping; .agents/test-authoring.md:16–17 requires a
  distinguishing observable. No exact existing coverage found.

Caveats:

- Distinct forwarded IDs and reply contents are checked. Those checks do not establish the claimed out-of-order premise.
  No shipped misrouting was demonstrated.
- The test does check distinct forwarded IDs and answer ownership. No shipped misrouting demonstrated. Same location as
  cli_installation_04_sec:p1:C3.
- Same-location duplicate of cli_installation_04_cor:p1:F2. Existing distinct-ID and ownership assertions remain useful;
  no shipped security defect established.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_04_cor:p1:F2`,
`cli_installation_04_sec:p1:C3`.

- `cli_installation_04_cor:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
- `cli_installation_04_sec:p1:C3`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.
