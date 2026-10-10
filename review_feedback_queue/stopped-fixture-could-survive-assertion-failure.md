# Stopped fixture could survive an assertion failure

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The stopped-process fixture has failures outside any active cleanup owner.

## Details

F287 — **definite** — `crates/farhelm-supervisor/src/service/sweep.rs:2454` — Stopped fixture could survive an assertion
failure

SIGSTOP precedes fallible polling before guard construction, and the fixture is stopped again after that guard is
explicitly dropped. Either interval can unwind with a suspended child whose nominal sleep cannot finish; private
descriptors also avoid the runner's ordinary leak check. Arm a separate unwind-safe kill-and-reap fixture owner
immediately after spawning and retain it through both stopped intervals.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/sweep.rs:2454–2475: SIGSTOP precedes fallible polling and the guard's
  construction.
- crates/farhelm-supervisor/src/service/sweep.rs:2477–2480: StoppedProcessGuard is created and explicitly dropped.
- crates/farhelm-supervisor/src/service/sweep.rs:2485–2510: the child is stopped again before further assertions and
  manual kill/wait.
- crates/farhelm-supervisor/src/service/sweep.rs:2411–2413: the fixture uses the same ordinary sleeper Child.
- crates/farhelm-supervisor/src/service/sweep.rs:746–752: the production guard resumes recorded identities; it does not
  own fixture termination.
- nextest-runner/src/runner/executor.rs:1475–1551 and unix.rs:29–47 at cargo-nextest-0.9.143: normal Unix completion
  does not reap arbitrary surviving group members.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- {"matched": null, "possible": "The runner timeout/leak configuration does not supply cleanup for this ordinary
  assertion-failure path."}

Caveats:

- No runtime reproduction was performed.
- Independent test location from handlers.rs:4012 and handlers.rs:4147.
- The two stop windows belong to this one editable test and need not become separate findings.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_cor_06:p1:C4`.

- `gap_supervisor_state_cor_06:p1:C4`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.
