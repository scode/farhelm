## What this was about

A test checks that preparing a spare checkout cleans up processes left by its startup shell. To tell the shell to
continue, the test writes to a named pipe. If the shell exits early, the old open could wait indefinitely for a new
reader and never reach the cleanup checks. Triage chose to fix it if the change was small enough to keep the existing
test structure.

## Things you should know

The test now retries a nonblocking pipe open for up to ten seconds. If the startup shell is gone and nobody reads the
pipe, the test fails at that step with a timeout naming the missing reader. The change uses the existing bounded wait
and fits the 30-line triage limit.

## Open questions and possible follow-ups

The early reader exit was identified by code review. How often it caused a hang in practice remains unmeasured.

## Pull request

- [#4390](https://github.com/scode/farhelm/pull/4390/changes) bounds the pipe release and archives the resolved
  feedback.

## Checks

- `cargo test --locked -p farhelm --test spare_builder spare_builder_completed_clone_retires_held_and_closed_pipe_descendants -- --exact`
  passed in recorder run `56d6baf7-0c8e-4b42-825c-534ebad722e4`. The existing test runs both variants, where the
  descendant keeps its output pipes open and where it closes them. The test also passed with main's new recorder
  ownership check in `041cc380-bb5c-40c7-bedc-4d1efda31e2b`.
- `cargo clippy --locked -p farhelm --test spare_builder -- -D warnings`, `cargo fmt --all -- --check`,
  `python -B scripts/check-test-sleeps.py`, dprint on the changed triage documents and `git diff --check` passed.
  `python3 scripts/check-agents-md.py` passed with one pre-existing archived citation warning. The Rust checks cover the
  test in `37422afa`. The rebase preserved the test source and reconciled the shared archive.
- The rest of the suite was skipped. The recorded test exercises the single changed fixture.

## Review gate

The required single source review found no issues. It confirmed that an absent reader reaches the existing bounded poll
when no reader opens the pipe and that the change stays within the agreed scope.
