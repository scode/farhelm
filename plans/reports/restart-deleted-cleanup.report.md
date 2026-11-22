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
