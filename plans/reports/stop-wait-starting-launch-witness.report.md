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

### Landing

#### What landed with it

launcher-repository-search-loading, the launcher's repository search fix, landed just before this plan in the same
round, and a large triage change (#4392) landed on main while the round's checks ran; both touch only other code and the
shared triage records. An independent reviewer and I read the changes against each other and found no interaction. The
landing made no fixes of its own to this plan.

#### Checks

This round landed two plans together: launcher-repository-search-loading (the desktop launcher's repository search) and
stop-wait-starting-launch-witness (one supervisor test). The landing ran the desktop app's GUI test suite, the
supervisor's tests of its shutdown wait, and the main lint (clippy) on a separate arm64 Linux test machine, and the
screenshot comparisons and small checks locally on x86_64, on both plans stacked together on main:

- `cargo nextest run -p farhelm-gui`, on the test machine (recorder run `b90372e6-9c53-4ddb-ac08-364a3b78c896`): all
  3226 tests passed.
- `cargo nextest run -p farhelm-supervisor --lib -E 'test(/stop_wait|pending_hold/)'`, on the test machine (recorder run
  `a94b80ea-5c52-44b5-97d2-88eb617e8378`): all 3 passed, including the changed test.
- `cargo clippy --all-targets -- -D warnings`, on the test machine, not through the recorder: passed.
- `python3 scripts/check-gui-goldens.py` (the screenshot comparisons), locally (recorder run
  `b19ba13c-2850-4bcd-aa39-b644e80d7250`): passed. No screenshot shows a search still loading, so the comparisons do not
  cover the new loading message; the GUI tests above do.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check` and
  `python3 releasing/check-changelog.py format` (formatting, test and changelog lints), locally: passed.

While these checks ran, a large triage change (#4392, review findings that cited removed UI code) landed on main. It
changes only the triage records and review queue, which the plans also edit, so each plan was rebased onto it with its
own entries kept and nothing else changed; the runs above still apply. Skipped: the workspace suite and the end-to-end
smoke test, since each change is confined to what the runs above cover; and Mac runs, since nothing here is
Mac-specific.

#### Things you should know that the report does not say

Nothing beyond the report.

#### What you should still do

Nothing.
