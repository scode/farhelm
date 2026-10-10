# Stop test can leave its fixture permanently stopped after failure

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Failure after stopping the Stop fixture could leave its child indefinitely suspended.

## Details

F203 — **possible** — `crates/farhelm-supervisor/src/service/handlers.rs:4015`;
`crates/farhelm-supervisor/src/service/handlers.rs:4012` — Stop test can leave its fixture permanently stopped after
failure

The test sends SIGSTOP without an unwind-safe child owner. A later panic before cleanup drops the child handle without
resuming or killing it; its nominal sleep cannot expire while suspended. The inspected runner path supplies no
ordinary-completion reap that closes this gap. No runtime failure was induced. Arm immediate kill-and-reap ownership
after spawn and before SIGSTOP for this Stop fixture.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/handlers.rs:4012–4023 holds a raw std::process::Child and stops it before
  fallible Supervisor construction at :4024–4026 and store setup at :4027–4059.
- crates/farhelm-supervisor/src/service/handlers.rs:4124 waits for the child only on successful completion.
- crates/farhelm-supervisor/src/procs.rs:1704–1709 explicitly transfers kill/wait ownership to the caller and returns a
  raw child.
- scripts/record-test-run.py:1408–1416 permits normal command completion without process-group cleanup when
  termination_reason is absent.
- crates/farhelm-supervisor/src/service/handlers.rs:4012–4026: Stop's fixture SIGSTOPs a raw Child before fallible
  supervisor construction.
- crates/farhelm-supervisor/src/service/handlers.rs:4147–4173: Restart independently repeats that ordering.
- crates/farhelm-supervisor/src/procs.rs:1709–1731: sleeper construction returns an ordinary Child, with private piped
  stdout and null stderr.
- crates/farhelm-supervisor/src/procs.rs:1801: the sleeper's finite lifetime requires its process to keep executing.
- scripts/record-test-run.py:1273–1278: runner-owned test groups are outside recorder ownership.
- nextest-runner/src/runner/executor.rs:1073–1109,1475–1551 at cargo-nextest-0.9.143: ordinary completion checks
  inherited output descriptors, not surviving process-group membership.
- nextest-runner/src/runner/unix.rs:29–47 at cargo-nextest-0.9.143: Unix Job ownership is inert.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- {"matched": null, "possible": "The timeout and leak settings at .config/nextest.toml:18–21 do not cover an ordinary
  panic with no inherited-output leak. TRIAGE_OUTCOMES.md:3883–3899 concerns production task ownership across
  cancellation, not fixture panic cleanup."}

Caveats:

- Pinned nextest descendant cleanup on an ordinary test failure was not established.
- SIGSTOP prevents the sleeper's normal timed completion until something resumes or kills it.
- Shares the Stop fixture with F1 but needs a separate ownership fix.
- No runtime reproduction was performed.
- The consequence concerns test-owned stopped processes.
- Split this aggregate into separate Stop and Restart fixture findings at handlers.rs:4012 and handlers.rs:4147.
- Pinned runner source was read from
  https://raw.githubusercontent.com/nextest-rs/nextest/cargo-nextest-0.9.143/nextest-runner/src/runner/.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_sec_06:p1:F2`,
`gap_supervisor_state_cor_05:p1:C3`.

- `gap_supervisor_state_sec_06:p1:F2`: confidence as filed: possible; suggested bucket as filed: other.
- `gap_supervisor_state_cor_05:p1:C3`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.
