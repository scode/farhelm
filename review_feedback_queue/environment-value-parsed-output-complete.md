# Environment value is parsed before its output is complete

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A split output line could make the environment test parse an incomplete value.

## Details

F182 — **possible** — `crates/farhelm/tests/e2e/restart_with_resume.rs:1554` — Environment value is parsed before its
output is complete

The wait can finish after seeing the value's prefix but before terminal delivery includes its remainder. Parsing then
reports empty or partial data even though the shell emitted the correct environment value. No split-delivery trigger was
reproduced, and reviewers dispute whether the safe retryable failure is filtered; this remains a possible test defect.
Wait for the subsequent readiness marker or a complete terminated line before parsing.

## Evidence and triage context

- crates/farhelm/tests/e2e/restart_with_resume.rs:1554 waits only for the marker prefix, then parses and rejects
  incomplete values.
- crates/farhelm/tests/e2e/harness.rs:1768 accepts a substring; :64 parses whatever suffix is currently present without
  requiring a terminator.
- crates/farhelm-fixtures/src/fake_agent.rs:820 writes the environment line before basic() emits its readiness marker.
- crates/farhelm/tests/e2e/restart_with_resume.rs:1554 waits for the prefix and immediately parses at :1555.
- crates/farhelm/tests/e2e/harness.rs:64 accepts an incomplete token.
- crates/farhelm-fixtures/src/fake_agent.rs:820 writes and flushes the complete short line before READY.
- review_feedback_queue/FILTER.md:24 excludes narrow-timing, safely retryable failures.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- No exact coverage identified. FILTER.md does not explicitly classify regression-test false failures as safely
  retryable product operations, so its ambiguous application is insufficient for dropping.
- FILTER.md:24, rare self-correcting/safely retryable failures.

Caveats:

- The split-delivery trigger was not reproduced.
- SPEC.md:1386-1396 requires the launch environment to reflect rc files; it does not require a terminal event to contain
  a whole line.
- Split delivery was not reproduced.
- Keep separate from F1: the initial observation and post-restart assertion are independently editable.
- Independent ui_a DROP/ui_b KEEP filter-applicability dispute remains unresolved. No split-delivery or later-timestamp
  interleaving was reproduced.
- No stream-boundary reproduction.
- The shared enclosing test also contains F1, but the readiness predicate is a separate editable defect.
- No actual split was demonstrated. Waiting for READY would still be a reasonable future edit when touching this test.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_07_cor:p1:F2`,
`cli_installation_07_sec:p1:F2`.

- `cli_installation_07_cor:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
- `cli_installation_07_sec:p1:F2`: confidence as filed: possible; suggested bucket as filed: other.
