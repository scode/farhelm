# The dead-tab regression cannot pass on ordinary macOS

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The dead-tab test fails on ordinary macOS before checking cleanup.

## Details

F163 — **definite** — `crates/farhelm-supervisor/src/service/ticker.rs:2548` — The dead-tab regression cannot pass on
ordinary macOS

The fixture unconditionally requires Linux process-filesystem liveness evidence and also uses a daemon-launch tool
absent from ordinary macOS. Supported-platform runs can therefore fail before establishing the intended tab-cleanup
result. Keep portable tab-reaping coverage and isolate the Linux daemon scenario, or provide an owned portable fixture
with platform-aware observations and an explicit cleanup premise.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/ticker.rs:2458–2488: the ungated fixture invokes setsid and waits for its PID
  publication.
- crates/farhelm-supervisor/src/service/ticker.rs:2546–2549: it unconditionally requires /proc/<pid>.
- crates/farhelm-supervisor/src/service/ticker.rs:1902–1903 and crates/farhelm-supervisor/src/service/mod.rs:97–99: no
  Linux-only module gate applies.
- SPEC_impl.md:2175–2184: macOS uses sysctl rather than /proc and has an additional platform-binary marker limitation.
- crates/farhelm-supervisor/src/service/ticker.rs:1902–1903 gates the enclosing module only on test; :2458–2459
  registers this test without a Linux condition.
- crates/farhelm-supervisor/src/service/ticker.rs:2482–2484 invokes setsid and waits for its child to create a PID file.
  dead_tab_running calls wait_for_dead_pane at :2442; :2180–2190 fails when that shell never exits.
- crates/farhelm-supervisor/src/service/ticker.rs:2546–2549 requires /proc/<pid> to exist before cleanup. That assertion
  fails on ordinary macOS even if setsid is supplied.
- crates/farhelm-testtrace-macros/src/lib.rs:55–63 emits an ordinary test wrapper with the original attributes; it
  supplies no platform exclusion.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- SPEC_impl.md:2181–2184 accepts a production marker-discovery residual, not a test that requires nonexistent procfs or
  an unavailable setsid command.

Caveats:

- No macOS execution was performed.
- This establishes a test defect, not a production tab-cleanup failure.
- Replacing only the procfs assertion would leave the daemon fixture's other platform assumptions unresolved.
- This is not evidence of a production tab-cleanup failure.
- Replacing the procfs assertion alone does not resolve every fixture assumption.
- Confirmed by source inspection and platform semantics, without macOS execution.
- This proves a test failure, not a failure of the product's tab cleanup.
- The nearby post-cleanup helper also uses procfs at ticker.rs:2224–2226, so replacing only the initial assertion would
  leave a vacuous macOS death check.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_cor_07:p1:F1`,
`gap_supervisor_state_sec_08:p1:F2`.

- `gap_supervisor_state_cor_07:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
- `gap_supervisor_state_sec_08:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
