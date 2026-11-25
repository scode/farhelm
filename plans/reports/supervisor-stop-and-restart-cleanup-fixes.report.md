# Supervisor Stop and Restart cleanup

## What this was about

When Restart failed after a session disappeared from Farhelm, it could leave processes running with no session to stop
them from. On Linux, a scope tracks the processes for one launch. A scope that looked empty could still receive a
late-starting process. The plan also asked for Stop to settle a pending launch before reporting success.

## Things you should know

- On Linux, a failed Restart now records cleanup for the exact attempt when the session row is gone. At the next
  supervisor startup, it settles the recorded host start before trusting a scope check. If a session or an outstanding
  Create has since claimed that same session ID, cleanup checks the old scope without sweeping processes from the
  restored session. If the restored row owns the same scope, the cleanup stays pending.
- I returned the Stop item to TODO under the plan's complexity rule. The existing Stop intent can be marked complete
  when the terminal goes missing, or cleared when the supervisor rediscovers a live host. Fixing the Stop reply alone
  would still let that intent disappear before the sweep was confirmed.

## Open questions and possible follow-ups

The Stop follow-up needs to preserve an unfinished sweep through supervisor restart and offer consent based on whether
the agent could still run. The TODO now records those constraints and the related case where a terminal is usable even
though the create confirmation could not be written.

## Pull requests

- [#5005](https://github.com/scode/farhelm/pull/5005/changes) keeps a failed Restart's cleanup retryable and updates the
  remaining Stop TODO.

## Checks run, reused and skipped

- `cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e -E <selection>` ran 20 focused cases at
  `b55b54a8` (recorder `454e175f-41f0-417d-941d-4b9354eb0ea8`). Nineteen passed across Restart cleanup, startup reclaim
  and session ID ownership. The late-start case timed out at its stale socket, and that failure is retained. The earlier
  attempt at `95caa576` stopped on a compile error before running tests (recorder
  `848801f5-a7d5-4118-9efa-79ae14b6d9b1`).
- `cargo nextest run -p farhelm-supervisor --lib -E 'test(=service::core::host_restart::tests::pending_restart_cleanup_checks_its_starter_before_its_scope)'`
  passed the repaired fixture at `c9135d78` (recorder `67bb397a-252a-463a-8c5d-7256a9e498ab`). That change touched only
  the fixture, so the other nineteen passing results still apply. These scope checks use a stand-in manager. The
  selected e2e checks drive the real supervisor and terminal hosts.
- `cargo clippy --locked -p farhelm-supervisor --all-targets -- -D warnings`, `cargo fmt --all -- --check`,
  `dprint check SPEC_impl.md TODO.md releasing/changelog.d/retry-failed-restart-cleanup.md`, `git diff --check`,
  `python3 releasing/check-changelog.py format` and `python -B scripts/check-test-sleeps.py` passed. These direct checks
  have no recorder IDs.
- I skipped the workspace suite and visual checks. The change is in the supervisor's startup and Restart paths, and the
  focused selection exercised their affected boundaries.
- Main advanced with the quick dialog work and queue metadata. It changed no supervisor code or relevant tests. The
  integration trial merged cleanly, so the selected test results still apply to the draft.

## Review gate

An independent source review identified a second-stage race after host shutdown, a missing private socket directory
check, misleading scope-only failure wording, and two fixture gaps. The code now settles the exact recorded start before
retiring it and refuses unsafe host directories. The recheck cleared the production changes and reservation case. The
final test-only change proves that the scope is absent before the starter is released and seen on the next attempt.
