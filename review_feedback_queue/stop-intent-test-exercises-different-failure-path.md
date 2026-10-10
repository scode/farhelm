# The stop-intent test exercises a different failure path

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The stop-intent test misses the live-agent transaction ordering.

## Details

F155 — **definite** — `crates/farhelm-supervisor/src/service/core.rs:22836` — The stop-intent test exercises a different
failure path

Its terminal-less fixture takes a classification branch rather than the live-agent stop path whose durable-intent write
it claims to protect. Moving signals before that write can therefore leave the test green. Provide an owned live agent
and terminal, inject failure in the intent transaction, and verify that the original process survives the refused Stop.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/core.rs:22783–22800: the stated test contract concerns intent recording before
  signals.
- crates/farhelm-supervisor/src/service/core.rs:22834–22842: the fixture has no terminal and corrupts the durable row.
- crates/farhelm-supervisor/src/service/core.rs:22868–22874: assertions accept Internal with 'nothing was killed'.
- crates/farhelm-supervisor/src/service/handlers.rs:829–841,902–915: terminal-less classification failure emits that
  same response.
- crates/farhelm-supervisor/src/service/sweep.rs:1880–1904: the live-agent path records StopRequested before reaping.
- crates/farhelm-supervisor/src/service/core.rs:22782 states the record-before-kill contract, but line 22836 creates an
  entry with no terminal.
- crates/farhelm-supervisor/src/service/handlers.rs:778 leaves no live pane and line 829 selects the dead-or-absent
  branch.
- crates/farhelm-supervisor/src/service/handlers.rs:902 emits the classification-write failure text asserted at
  core.rs:22873.
- crates/farhelm-supervisor/src/service/sweep.rs:1886 contains the live-agent intent-before-signal ordering the test
  never executes.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- No matching queue/ledger coverage. This satisfies the concrete false-pass threshold in the task and the fixture/oracle
  rules in .agents/test-authoring.md.

Caveats:

- Existing assertions still cover classification-failure reporting.
- No current production ordering violation is alleged.
- The test does cover classification-write failure reporting.
- No production ordering defect or runtime failure is alleged.
- Production currently records StopRequested before signaling.
- No mutation test or runtime test was run.
- Production currently has the correct ordering.
- No mutation test.
- Preserve the existing dead-pane classification coverage when correcting or separating the intended live-agent test.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_cor_03:p1:F2`,
`gap_supervisor_state_sec_04:p1:F2`.

- `gap_supervisor_state_cor_03:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
- `gap_supervisor_state_sec_04:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
