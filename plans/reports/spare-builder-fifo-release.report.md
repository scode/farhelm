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

### Landing

#### What landed with it

Two other small plans landed just before this one in the same round: the installer's command-line fix for an
installation without an ID and the launcher's folder warning fix. Since this plan's work started, main had gained only a
change to the developer test tools, which touches none of this plan's code. An independent reviewer and I read the three
plans' changes against each other. The only files they share are the triage records for review feedback, and each plan's
edit to those survived. The landing made no fixes of its own to this plan. The test now waits at most ten seconds for
the startup shell to start reading the pipe that releases it, and fails after those ten seconds rather than hanging if
the shell has exited.

#### Checks

This round landed three small plans together: installer-expect-hyphen-value (the installer's command line),
launcher-folder-refusal-order (the desktop launcher's folder warning) and spare-builder-fifo-release (one test). The
landing ran the affected tests, the installer's shell test suite and the main lint (clippy) on a separate arm64 Linux
test machine, and the small checks locally on x86_64, on all three plans stacked together on main:

- `cargo nextest run -p farhelm -p farhelm-gui -E 'binary(farhelm) | binary(spare_builder) | test(/folder_check::/)'`,
  on the test machine (recorder run `b91fdebc-623b-4755-9043-a5871f8e60f8`): all 151 selected tests passed, including
  each plan's changed or new test.
- `cargo build -p farhelm && bash scripts/test-install-sh.sh --farhelm target/debug/farhelm` (the Mac installer's shell
  tests, driven on Linux), on the test machine, not through the recorder, which the landing's script ran it outside of
  by oversight: all 547 checks passed.
- `cargo clippy --all-targets -- -D warnings`, on the test machine: passed.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check` and
  `python3 releasing/check-changelog.py format` (formatting, test and changelog lints), locally: passed.

Skipped: the workspace suite, the desktop app's other tests and the end-to-end smoke test, since each change is confined
to the code its tests cover; and Mac runs, since the installer's shell tests drive the Mac installer's script (they do
not settle installer-expect-hyphen-value's open question about real updates, which its note repeats).

#### Things you should know that the report does not say

Nothing beyond the report.

#### What you should still do

Nothing.
