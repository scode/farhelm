# Restart test has the same stopped-child cleanup gap

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The Restart fixture separately risks leaving a suspended child behind.

## Details

F204 — **possible** — `crates/farhelm-supervisor/src/service/handlers.rs:4150`;
`crates/farhelm-supervisor/src/service/handlers.rs:4147` — Restart test has the same stopped-child cleanup gap

This independently created child is stopped before any unwind-safe kill-and-reap owner protects it. A failure before the
intended sweep can drop its handle while SIGSTOP prevents its nominal sleep from finishing. Its unique marker prevents
cross-invocation targeting but does not ensure cleanup. No runtime reproduction was performed. Arm immediate child
ownership before stopping it at this Restart fixture site.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/handlers.rs:4147–4158 spawns a raw child and sends SIGSTOP before fallible
  setup.
- crates/farhelm-supervisor/src/service/handlers.rs:4163–4173 constructs the supervisor afterward; :4174–4212 performs
  further fallible store setup.
- crates/farhelm-supervisor/src/service/handlers.rs:4244–4252 can panic while waiting for the restart to begin.
- crates/farhelm-supervisor/src/procs.rs:1704–1709 leaves kill/wait ownership with the caller.
- scripts/record-test-run.py:1408–1416 supplies no unconditional normal-completion descendant cleanup guarantee.
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

- Ordinary-failure cleanup by pinned nextest remains unverified.
- Test-only resource leak; no runtime reproduction.
- Separate editable site from the Stop fixture.
- No runtime reproduction was performed.
- The consequence concerns test-owned stopped processes.
- Split this aggregate into separate Stop and Restart fixture findings at handlers.rs:4012 and handlers.rs:4147.
- Pinned runner source was read from
  https://raw.githubusercontent.com/nextest-rs/nextest/cargo-nextest-0.9.143/nextest-runner/src/runner/.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_sec_06:p1:F3`,
`gap_supervisor_state_cor_05:p1:C3`.

- `gap_supervisor_state_sec_06:p1:F3`: confidence as filed: possible; suggested bucket as filed: other.
- `gap_supervisor_state_cor_05:p1:C3`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.
