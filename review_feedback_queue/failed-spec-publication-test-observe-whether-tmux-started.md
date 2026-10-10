# Failed-spec-publication test does not observe whether tmux started

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Failed-publication tests cannot prove that no external window started.

## Details

F200 — **definite** — `crates/farhelm-supervisor/src/service/core.rs:23626` — Failed-spec-publication test does not
observe whether tmux started

A regression can create a tmux window before the launch-spec write fails, then roll back internal bookkeeping to the
empty state the test accepts. Empty records do not distinguish never launched from launched without a record. Verify the
specific publication refusal and directly record or assert absence of tmux creation attempts.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/core.rs:23563 claims that the test proves no tmux window is created before
  successful spec publication.
- crates/farhelm-supervisor/src/service/core.rs:23620 accepts any Error and checks only in-memory and durable session
  records.
- crates/farhelm-supervisor/src/service/core.rs:9317 abandons the launching record on SpawnFailure::Spec without the
  test observing external tmux creation.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- No matching disposition. This is an inadequate oracle for an explicit ordering contract, not a request for generic
  extra coverage.

Caveats:

- This establishes an inadequate oracle for a specific stated regression, not a current production launch-order
  violation.
- No runtime mutation test was performed.
- No current production launch-order violation established.
- No mutation test.
- A corrected test should observe creation attempts and the intended publication refusal.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_sec_04:p1:F3`.

- `gap_supervisor_state_sec_04:p1:F3`: confidence as filed: definite; suggested bucket as filed: other.
