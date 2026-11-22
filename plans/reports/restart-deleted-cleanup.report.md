# Retry Restart cleanup after session deletion

## What this was about

On Linux, deleting a session while Restart is launching its replacement can leave the replacement agent's processes
running. If cleanup could not confirm those processes had exited, Farhelm could lose track of them when the session
disappeared.

Farhelm now remembers the exact group of processes it could not clean up. On the next supervisor start, it retries
cleanup even if no sessions remain. It keeps the record if it still cannot confirm cleanup.

## Things you should know

The retry uses the existing startup time budget. It runs alongside the check for leftover terminal hosts, so a stalled
host check cannot prevent the process cleanup from getting a chance to run.

## Open questions and possible follow-ups

None.

## PRs

- Preserve and retry the cleanup when the replacement outlives its deleted session.
  [#4625](https://github.com/scode/farhelm/pull/4625/changes)

## Checks

- Ran the isolated `python -B scripts/check-test-sleeps.py`, `cargo fmt --all -- --check`,
  `cargo clippy --all-targets -- -D warnings`, Markdown formatting, the changelog format checker and `git diff --check`.
- Ran
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(=service::core::host_restart::tests::a_deleted_restarts_scope_is_retried_at_startup_after_its_host_is_gone)'`.
  The regression passed after correcting a test fixture that failed to compile in the first attempt. Retry run
  `b71c2bd9-1e09-4aba-ab35-7d8b7724bf9c`, initial run `57ce6ae0-7a1e-4abb-96fa-0bf3ae91b42a`.
- Ran
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(/^store::tests::/) | test(/^service::core::host_restart::tests::/) | test(/^service::core::host_reclaim::tests::/) | test(a_second_serve_on_the_same_state_dir_is_refused)'`.
  All 147 passed. Run `de641087-4514-4567-a99d-ceddf2d998b4`.
- Ran `cargo nextest run -p farhelm --test e2e -E 'test(/^host_relaunches::/) | test(/^host_reclaim::/)'`. All 34 passed
  with local terminal hosts. Run `5faa0455-f150-4650-85b5-309afc2ae4be`.
- The runtime commands used the pinned nextest and `python3 scripts/record-test-run.py`. The draft has the same tree as
  the tested revision. Validation came from this execution.
- Skipped the broader battery because the selected runs cover storage, Restart, startup ownership and terminal-host
  cleanup. The regression simulates a failing systemd scope manager through the test fixture. The local end-to-end suite
  exercises real terminal hosts.

## Review gate

The independent source reviewer verified the repairs that let cleanup proceed alongside the host check, clarify the
Restart contract and check that signals target only the recorded group of processes. The separate wording review passed.

### Landing

#### What landed with it

The plan's PR (#4625) landed second in this round, after the change that keeps working container images when Rebuild
fails and before the harness drift test's resume check and the screen fixture scrubber. Since the plan was written, main
had gained refactors of the desktop window's code and a change to how the helm learns whether a host setup command
succeeded over SSH, none of which touch the supervisor's Restart or Delete. The rebase conflicted only in TODO.md, where
each plan removes its own entry.

An independent reviewer checked how the leftover cleanup is remembered and the Restart and Delete race, and found
nothing to fix: the retry can only reach the replacement agent's own process group that it recorded, never a live
session's processes, and a supervisor crash in the moment before the record is written falls back to the old behavior,
where those processes could be left running untracked.

#### Fixes made while landing

None.

#### Checks

The round landed changes to the helm's container recipe rebuilds and the supervisor's Restart cleanup, plus two test
tools, so the landing ran the Rust workspace suite, the lint runs and the two tools' own tests, once, on the four plans
stacked together. No desktop code changed (the new Rebuild wording in Settings and the launcher comes from the helm), so
the desktop app's tests, the end-to-end smoke test and the reference screenshots were not run. The worker runs were on a
separate arm64 Linux test worker with a systemd user manager, rootless Podman and SSH to itself; the rest ran locally.

On the four plans as delivered, rebased onto main and stacked:

- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `03875ce4-e83a-47da-87cb-b3c4ee02da69`): 8349 of 8350 passed, none printed a runtime `SKIPPED`, and the 86 tests
  marked ignored did not run, as on every run. The real Podman recipe build and launch test ran and passed. The one
  failure was the supervisor test that refuses session titles with control characters, which timed out after ten seconds
  waiting for its own create reply while the suite ran around it. It exercises neither supervisor startup nor Restart,
  the parts of the supervisor the Restart cleanup plan changes, it is not a known flake, and it then passed 30 times in
  a row on the same worker, each attempt a separate recorder run inside one worker session (worker session
  `687080cb-021d-499c-a209-af6f89149001`). One timeout is not enough to record it as a flake.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker:
  passed. Lints are not runtime tests, so they have no recorder run.
- `python3 -B scripts/test-run-harness-drift.py`, `python3 -B scripts/test-capture-agent-screens.py`,
  `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format` and `python3 scripts/check-agents-md.py`, locally: passed.

Skipped: the real-agent harness drift tests, which spend vendor turns and run only when asked; macOS, since nothing in
the round builds only on a Mac; CI, which runs only on demand.

#### Things you should know that the report does not say

- Nothing beyond the report.
