# Verify shutdown waits for a starting launch

## What this was about

When a launch starts as Farhelm shuts down, the supervisor needs to wait for it to register the app version its terminal
will use. An installer can remove that version before the new terminal has secured it if the wait ends too soon. The
previous test could miss an early return. The revised test runs the wait with registration still held back, asserts that
it is pending, then registers the launch and checks that the same wait finishes.

## Things you should know

This is a test change. The assertion exercises the reported regression, which would return immediately when there are no
registered launches yet. Moving registration into the test task removes its dependence on a fixed delay.

## Open questions and possible follow-ups

A separate test could hold registration back across later checks if a delayed early return becomes a concrete concern.

## Pull requests

- Make shutdown's test observe the wait before launch registration.
  [#4391](https://github.com/scode/farhelm/pull/4391/changes)

## Checks

- `cargo nextest run --locked -p farhelm-supervisor --lib -E 'test(the_stop_wait_covers_launches_still_starting) | test(the_hold_wait_waits_for_a_host_agents_child)'`
  passed both cases with four nextest slots and zero retries on the source captured in `f8778d4b` (recorder
  `e6a4a2d3-7ffe-483f-af09-ab55e95116ab`). The earlier attempt `bd2ea3c2-5553-40a2-be76-6c8067b5b8d4` was interrupted by
  SIGTERM during compilation, before either test ran. The final PR reuses the successful run because the rebase
  preserved the supervisor source byte for byte.
- `rustfmt --edition 2024 --check crates/farhelm-supervisor/src/service/core.rs` and
  `python -B scripts/check-test-sleeps.py` passed on the unchanged Rust source. Targeted `dprint check`,
  `python3 scripts/check-agents-md.py` and `git diff --check` passed after rebasing. The instruction checker reported
  its existing archived citation warning. The workspace suite was skipped because the focused pair exercises the test
  change and its neighboring shutdown case.

## Review

The independent source and wording reviews passed. Source review walked through the immediate-return regression and
confirmed that it fails the new assertion before the test can register the launch.
