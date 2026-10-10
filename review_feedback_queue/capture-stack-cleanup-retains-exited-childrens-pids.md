# Capture-stack cleanup also retains exited children’s PIDs

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Capture teardown could signal an unrelated process after a child exits.

## Details

F67 — **possible** — `e2e/readme-hero/start-stack.sh:123-124` — Capture-stack cleanup also retains exited children’s
PIDs

Screenshot and video cleanup retain numeric supervisor identifiers until the helm exits, without tracking early
supervisor exit or validating identity at signaling. If an exited child's number is reused before teardown, cleanup
could terminate another same-account process. No reuse sequence was reproduced. Track child exits and verify that every
remaining cleanup target belongs to the capture before sending a signal.

## Evidence and triage context

- e2e/readme-hero/start-stack.sh:150-160 records the local supervisor PID and remote PID array.
- e2e/readme-hero/start-stack.sh:123-124 signals those numbers during final cleanup; :219 waits only for the helm.
- e2e/readme-hero/stage.ts:235-245 can keep waiting for host readiness for 60 seconds after a supervisor failure.
- e2e/readme-video.config.ts:53-60 uses this same capture-stack implementation.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- SPEC_impl.md:2194-2205 excludes identifiers carried through these asynchronous waits. No exact Planned, BUGS, queue or
  ledger cover found.

Caveats:

- Requires early supervisor exit and reuse before cleanup. No deliberate termination is necessary, but the sequence was
  not reproduced.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_05_sec:p2:F2`.

- `test_infrastructure_05_sec:p2:F2`: confidence as filed: possible — requires an early supervisor exit followed by PID
  reuse before cleanup; suggested bucket as filed: highest.
