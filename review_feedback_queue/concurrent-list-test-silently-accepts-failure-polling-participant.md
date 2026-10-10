# The concurrent-list test silently accepts failure of its polling participant

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The concurrent-list test ignores failure of its observer.

## Details

F131 — **definite** — `crates/farhelm/tests/e2e/boot_id_durable_outcome.rs:657` — The concurrent-list test silently
accepts failure of its polling participant

The polling participant can fail immediately without propagating that failure to the test result. The operation under
test can then pass with no concurrent observer running, silently removing the scenario the test claims to exercise.
Propagate polling failures and establish successful polling during the operation, rather than treating a failed
participant as successful concurrency coverage.

## Evidence and triage context

- boot_id_durable_outcome.rs:653–663 creates the polling client, breaks on any list error, and returns unit. :665 checks
  only task panic. :667–685 checks final/durable state through other clients without requiring any successful poll.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- The stated test contract is boot_id_durable_outcome.rs:627–647. .agents/test-authoring.md:6–9 requires the fixture
  premise. No matching Planned, BUGS.md, queue, ledger, or filter coverage found.

Caveats:

- No polling failure was reproduced. Successful overlap is a separate scheduling question; the established defect is
  swallowing participant failure.
- No polling failure reproduced. Successful overlap remains a separate question; swallowed failure is directly
  established.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_04_cor:p1:F3`.

- `cli_installation_04_cor:p1:F3`: confidence as filed: definite; suggested bucket as filed: other.
