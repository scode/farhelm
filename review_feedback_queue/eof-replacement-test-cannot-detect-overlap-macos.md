# EOF replacement test cannot detect overlap on macOS

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The replacement-entry test cannot detect old/new client overlap on macOS.

## Details

F192 — **definite** — `crates/farhelm-supervisor/src/service/terminals.rs:3569` — EOF replacement test cannot detect
overlap on macOS

At replacement entry, the observer reports the old child gone because its Linux process-filesystem path is unavailable,
regardless of actual lifetime. The test therefore misses precisely the cleanup-before-replacement regression it
describes. Observe the old child's lifetime portably inside the replacement callback and establish the live-fixture
premise before exercising EOF handling.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/terminals.rs:3369 creates a process that closes stdout and then sleeps.
- crates/farhelm-supervisor/src/service/terminals.rs:3554 constructs a procfs path; :3569 uses its absence at
  replacement entry; :3580 asserts that observation.
- terminals.rs:3369-3377 starts a child that closes stdout and remains alive; :3554 constructs /proc/<pid>; :3569 tests
  its absence inside the replacement opener; :3580-3582 accepts that observation.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- No exact coverage identified; accepted temporary loss of a sink does not cover this false oracle.
- SPEC_impl.md:2176 recognizes the platform difference. No exact Planned, BUGS, queue or ledger coverage found.

Caveats:

- Test-only defect; no current production overlap established. The fixture naturally exits after 30 seconds, so a
  replacement test should also establish its live premise.
- Keep separate from F1: this assertion observes replacement entry rather than orderly shutdown return.
- No current production overlap established.
- No production overlap demonstrated. This independently editable test site remains separate from the other procfs
  assertions.
- A test defect, not evidence that production replacement presently overlaps the old client. Distinct editable boundary
  from F1 and F3.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_runtime_sec_02:p1:F2`, `sr_systems:p2:F2`.

- `gap_supervisor_runtime_sec_02:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
- `sr_systems:p2:F2`: confidence as filed: definite; suggested bucket as filed: other.
