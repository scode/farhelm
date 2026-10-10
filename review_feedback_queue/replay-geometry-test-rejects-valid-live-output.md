# replay-geometry test rejects valid live output

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The replay-geometry test could mistake live output for a bad replay.

## Details

F264 — **possible** — `crates/farhelm/tests/e2e/session_lifecycle.rs:1966`;
`crates/farhelm/tests/e2e/session_lifecycle.rs:1943-1969` — replay-geometry test rejects valid live output

Input echo can satisfy its first wait before the application echo is complete. A delayed application echo then enters
the buffer checked as replay, producing a capture-before-resize failure despite correct attachment geometry. The delay
sequence was not reproduced. Wait for completed application echo before detaching, check wrapping on the replay alone,
and collect later barrier output separately.

## Evidence and triage context

- crates/farhelm/tests/e2e/session_lifecycle.rs:1942-1947 sends the payload, waits only for its unqualified text, then
  detaches.
- crates/farhelm-fixtures/src/fake_agent.rs:1498-1504 reads canonical input lines; :1574-1576 subsequently writes the
  full payload contiguously inside its colored application echo.
- crates/farhelm/tests/e2e/harness.rs:188-197 returns initial replay separately from the stream after ReplayComplete.
- crates/farhelm/tests/e2e/session_lifecycle.rs:1949-1964 initializes replay with second_replay, then appends subsequent
  output while awaiting the application barrier.
- crates/farhelm/tests/e2e/harness.rs:1725 appends received data to the supplied buffer; :1767-1769 uses an ordinary
  substring predicate.
- crates/farhelm/tests/e2e/session_lifecycle.rs:1966-1970 rejects a contiguous payload anywhere in that combined buffer.
  SPEC_impl.md:1063-1066 distinguishes captured snapshot bytes from later live output.
- crates/farhelm/tests/e2e/session_lifecycle.rs:1946 accepts bare payload text before detach; :1954-1964 appends
  post-replay output to the replay buffer; :1966-1970 rejects contiguous payload bytes anywhere in it.
- crates/farhelm-fixtures/src/fake_agent.rs:1574-1576 emits the payload contiguously in the later application echo.
- crates/farhelm/tests/e2e/harness.rs:188-197 already separates initial replay from subsequent stream events.
- The delayed-application scheduling premise remains unverified, so retain as possible rather than reject.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- review_feedback_queue/FILTER.md:24-34 potentially concerns safely retriable failures, but its applicability to false
  regression-test failures is uncertain.
- SPEC.md:1197-1200 specifies terminal sizing and replay fidelity; SPEC_impl.md:1063-1066 specifies replay/live cutover.
  Neither accepts applying a rendered-replay wrapping assertion to live application bytes.

Caveats:

- The application-delay interleaving was not reproduced.
- This establishes a possible false test failure, not incorrect production resizing.
- The omitted geometry candidate in cli_installation_07_sec describes the same site and mechanism.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_07_cor:p2:F1`,
`cli_installation_07_sec:p2:C2`.

- `cli_installation_07_cor:p2:F1`: confidence as filed: possible; suggested bucket as filed: other.
- `cli_installation_07_sec:p2:C2`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.
