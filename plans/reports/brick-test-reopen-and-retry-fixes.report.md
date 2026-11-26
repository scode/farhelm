## What this was about

After repairing a test fixture that made a required P0 release check unavailable, the operator can use
`retry-unavailable` with the reason and carry on testing the same guest pair. The retry used to refuse when the harness
had automatically skipped later checks because of that unavailable result. It now archives the unavailable observation,
the reason for replay and only the later skips that depended on that observation.

The bonus reopen check asks whether the original remote workload keeps running when Farhelm quits and relaunches. Two
readings of a counter from the same desktop lifetime could previously pass without a relaunch. The run now records the
running candidate at the first sample, an acknowledged native Quit request, observed app exit and managed launch with
fresh process IDs, and the second sample. Guest finalization, host import and saved verdicts check that the receipt
matches both exported samples.

## Things you should know

If the installer has already reached the upgrade, the old-session test cannot start its baseline sessions on the
previous release. Widening the run after a retry keeps that work skipped, and feedback still waits for every required P0
check. Older saved skips without a recorded cause prevent retry, as agreed during planning.

A saved reopen result without the transition receipt remains in the record and receives a could-not-run verdict under
the new coordinator. The historical manual evidence format is documented.

## Open questions and possible follow-ups

A real Mac guest run still needs to establish that Command-Q reaches Farhelm, Farhelm exits and relaunches, and the
second counter sample is recorded against the new app. The portable checks exercised the bridge's construction and
forwarding of the native request.

## PRs

- #5023 makes unavailable brick-test checkpoints retryable. [Diff](https://github.com/scode/farhelm/pull/5023/changes)
- #5025 binds the reopen check to a fresh app process. [Diff](https://github.com/scode/farhelm/pull/5025/changes)

## Checks

- The recorded portable suite `python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py' -q` passed all
  328 tests on `3aa8900e` (run `b6817061-629a-4253-b6eb-f4eb87f89012`) and all 332 on `c4a9d59e` (run
  `461d2912-7680-43ce-8daf-f25296f3ae31`). The rebased draft heads `879baa54` and `215d4aa6` have identical brick-test
  files to their respective tested revisions.
- The same recorded unittest discovery command with `-p test_guest_operations.py -k reopen -v` passed three guest cases
  for Quit binding, missing Quit and stale samples (run `79c40b5b-6cc7-48bb-8e62-0b5bb9f6ff92`). With
  `-p test_brick_test.py -k native_reopen -v` the native host import and saved verdict test passed (run
  `b888a0ad-490c-45dd-85e6-a3fb297f210f`).
- `git diff --check`,
  `dprint check .agents/run-all-tests.md .agents/validation.md TODO.md releasing/mac-vm-test/BRICK-TEST.md` and
  `python3 scripts/check-agents-md.py` passed. The last check reported one existing archive cross-reference warning.
- No earlier plan's runtime evidence was reused. Live Mac testing and release gates were outside the approved test
  scope. The changes affect the Python harness and its instructions, so the Rust, desktop and hosted CI suites were
  skipped.
- Rebased onto `a1090987` after the agent-drift work landed. The conflicts were adjacent TODO entries. The resolution
  kept the landed entries and the two brick-test removals. The portable results were reused because the harness files
  and test recorder were identical. Markdown formatting, the instructions check and the diff whitespace check ran again
  after rebasing.

## Review gate

Each draft passed an independent implementation review and follow-up. The retry review led to regression tests for
widening after a spent old-session slot and independently skipped feedback on a released fixture. The reopen review led
to binding the Quit request, exercising native host import and documenting historical manual evidence.
