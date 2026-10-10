# The shutdown race test releases its sink before shutdown starts

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The shutdown race test starts after the critical transition is over.

## Details

F164 — **definite** — `crates/farhelm-supervisor/src/service/teardown.rs:3882` — The shutdown race test releases its
sink before shutdown starts

The fixture releases its output client's last ownership lease before the spawned shutdown begins inspecting the
registry. The cleanup worker is already registered, so even split observations can wait for it and pass. Control the
interleaving so the final lease is released between the observations that a split implementation would perform, and
demonstrate rejection of that implementation.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/teardown.rs:3841–3850: the contract concerns losing ownership between live-sink
  and reaper observations.
- crates/farhelm-supervisor/src/service/teardown.rs:3875–3884: shutdown is spawned and the lease synchronously dropped
  before the first yield.
- crates/farhelm-testtrace-macros/src/lib.rs:29–34,248–253 and crates/farhelm-testtrace/src/lib.rs:1131–1133: the
  default runtime is current-thread.
- crates/farhelm-supervisor/src/service/terminals.rs:1116–1154: lease destruction publishes Reaping synchronously.
- crates/farhelm-supervisor/src/service/terminals.rs:1788–1811: production currently takes an atomic registry snapshot.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- TRIAGE_OUTCOMES.md:1515–1518 records the production snapshot fix and claimed regression evidence, not a decision
  accepting this current test gap.
- review_feedback_queue/shutdown-expiry.md:19–24 and TRIAGE_OUTCOMES.md:7037–7064 concern timeout and process exit, a
  separate site and trigger.

Caveats:

- The production snapshot is currently atomic.
- No mutation test was run.
- This finding does not establish a current process-loss race or require redesigning production shutdown.
- The test correctly covers waiting for an already-published reaper.
- Production currently performs the atomic snapshot.
- No mutation test or current process-loss race was established.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_cor_07:p1:F2`.

- `gap_supervisor_state_cor_07:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
