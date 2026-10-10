# Lock test does not establish that the contender reached the lock

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The lock test can pass without any contender waiting behind the lock.

## Details

F158 — **definite** — `crates/farhelm-supervisor/src/service/core.rs:25036` — Lock test does not establish that the
contender reached the lock

The second task may not reach acquisition before the first guard is dropped. Its unfinished state then proves only late
scheduling, followed by uncontended acquisition and successful pruning. The claimed exclusion and handoff are not
established. Wait under a timeout for the second claim's explicit reached-lock signal before checking exclusion and
releasing the first guard, and bound handoff completion.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/core.rs:25030–25052: the test spawns a waiter, yields once, checks is_finished,
  drops the first guard, then checks eventual ownership/pruning.
- crates/farhelm-supervisor/src/service/core.rs:1797–1828: the existing arrival observer explains why an unpolled
  contender is indistinguishable from a blocked contender.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No particular scheduler run was reproduced.
- No production locking failure is alleged.
- No particular late-poll run was reproduced.
- No current production locking defect is alleged.
- The existing arrival observer provides the missing boundary.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_cor_04:p1:F2`.

- `gap_supervisor_state_cor_04:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
