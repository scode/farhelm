# Release builds fail the held-stdin hook test

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The held-input hook test falsely fails release binaries.

## Details

F140 — **definite** — `crates/farhelm/tests/e2e/hook_identity.rs:1209` — Release builds fail the held-stdin hook test

The test expects a debug-only shortened timeout, but release-profile hooks retain the intentional thirty-second
production wait. Its observation deadline kills a correctly waiting release child before that timeout can expire. Adjust
the deadline to account for the production budget when the child does not support the debug override, keeping premature
termination distinct from a genuine timeout regression.

## Evidence and triage context

- hook_identity.rs:1019 imposes seven seconds and :1023,1053 requests a five-second child override. :1209 keeps stdin
  open; :1107–1113 fails after seven seconds. main.rs:1339–1347 compiles out that override without debug_assertions.
  hook.rs:180 sets thirty seconds, and :404,414 waits for EOF or the budget.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- No exact Planned, BUGS.md, queue, ledger, or filter coverage found. The test's own timeout contract and child-only
  override assumptions are hook_identity.rs:1013–1023.

Caveats:

- No release-profile execution. The defect is conditional on a binary without debug_assertions and does not affect
  ordinary debug execution.
- No release-profile execution. Applies when the selected Cargo-built binary lacks debug_assertions, not ordinary debug
  runs.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_06_sec:p1:F1`.

- `cli_installation_06_sec:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
