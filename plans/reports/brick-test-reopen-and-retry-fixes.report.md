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

### Landing

#### What landed with it

This plan landed second in its round, right after the Restart and hook warning fixes
(`hooks-restart-and-antigravity-fixes`), which share no code with the brick test's tools. Since its PRs were based, main
gained the Settings triage round, which touches nothing here. The rebase applied without conflicts.

#### Fixes made while landing

- None. The landing's independent review found no blocker or should-fix item: a retry discards only the skips that the
  retried check caused, older skips without a recorded cause still refuse it, and the reopen check's samples are tied to
  an observed Quit and a new app process at every point that reads them.

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. All the checks below ran
on both plans' PRs stacked in queue order on main as it stood at the start of the landing (`62848f0d`), which applied
without conflicts; every file's change was exactly what the executors pushed until the one test fix inside #5028.

- The workspace Rust suite, `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `5185ff46-5722-4079-b858-27e3a2828d51`), before the fix: 8743 of 8744 passed, with no runtime `SKIPPED` lines. The one
  failure was the Delete fail-closed test, fixed inside #5028 as the Restart and hook warning plan's "Fixes made while
  landing" describes.
- After the fix, that test five times in a row,
  `cargo nextest run -p farhelm --test e2e -E 'test(=session_lifecycle::delete_fails_closed_when_a_launch_artifact_cannot_be_removed)'`,
  on the test worker (recorder runs `1b63cc37-c937-48e3-af55-68360ef893a6`, `aa7fc04d-ba16-4bbc-8063-d5ecb210951f`,
  `1a41ad24-07bf-4d0c-8466-0813a9d733b1`, `2dc96b26-8956-47b4-9e8c-009deb499ee9` and
  `d2cbf0dd-7a20-4515-835e-d07680dda566`): passed every time; and its whole module,
  `cargo nextest run -p farhelm --test e2e -E 'test(/^session_lifecycle::/)'` (recorder run
  `2690ad56-caba-4088-ae38-3a4f23956e80`): all 80 passed. The fix changes only that test's setup, so the rest of the
  workspace run still applies; `cargo clippy -p farhelm --all-targets -- -D warnings`, which lints that test, passed on
  the test worker after the fix.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets --features agent -- -D warnings`, on the test worker: passed.
- The desktop app's end-to-end smoke, `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run
  `8cb7bdf6-b1be-452c-95cd-56b25b487ab0`): of its 49 steps, 48 passed, including Restart, and one (paste) was skipped by
  design on Linux.
- The desktop app's screenshot comparison, `python3 scripts/check-gui-goldens.py`, all scenes, locally (recorder run
  `204aa932-5b38-4989-8d30-1b97d963a72c`): passed, including the session header and card scenes.
- The release brick test harness's portable tests,
  `python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py'`, locally (recorder run
  `307cd69e-d7f1-400c-b154-a64c3f19997a`): all 332 passed.
- `python3 scripts/check-agents-md.py`, `python3 releasing/check-changelog.py format`, `cargo fmt --all -- --check`
  (before and after the fix) and `python -B scripts/check-test-sleeps.py`, locally: passed. `dprint check`, locally:
  these plans' files pass; it fails only on over-long lines in the triage outcomes archive, which main has had since an
  earlier triage record (#4978) landed. No PR fixes them yet; a formatting-only one would, and I recommend it, as
  earlier rounds' notes did.

Reused across the fix: every run above apart from the reruns named. The executors' runs were focused selections on older
bases, and everything they covered ran again above.

Skipped: the tests of the desktop app's outer shell (`cargo nextest run -p farhelm-desktop`), since nothing in it
changed (the app's windows, including the session header and recovery card, live in its GUI crate, whose tests ran in
the workspace suite); a real Mac brick test run, which is a release-time check (the brick test plan's report says what
it still has to show); a native macOS pass of the right-to-left hook error text and its screen reader output, which the
Restart and hook warning plan's report leaves unverified; a live Antigravity run, which spends vendor turns; and CI,
which runs only on demand.

#### What the report says that is no longer true

- Nothing.

#### Things you should know that the report does not say

- **The brick test still tells its guest to dismiss the startup Welcome card**, which the next candidate no longer shows
  now that Welcome moved into the empty main pane (the landing notes of `welcome-empty-state` describe it). This plan
  does not touch that instruction or make it worse, but the next brick test of a candidate will meet it, so it is worth
  fixing before then.
- **Smaller points the review noticed.** The process check the reopen fix shares with the update-relaunch check now also
  refuses an update-relaunch result whose list of processes from before the relaunch is empty, which is not documented
  or tested; and some reasons the harness writes when it skips a check automatically are worded differently than before.
  Neither needs a follow-up on its own.
