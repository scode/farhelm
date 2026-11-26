### What this was about

Triage approved nine fixes to the opt-in agent drift suite and the release brick-test coordinator. The drift suite could
copy refreshed login files back while a test session might still be alive, or report a pass after a copy failed. It
could also silently omit a requested harness. Other checks could miss the first-prompt capture boundary, fail when the
suite tried to reopen the Cursor test chat by ID, or mistake inherited Claude trust and unrelated Claude writes for the
behavior under test. The brick-test coordinator could strand two prepared Mac guests when a stop-after-P0 request
excluded their P1 tests.

### Things you should know

The drift suite waits for confirmed session deletion and terminal shutdown before writing back login files. It continues
trying the remaining files after an individual write failure and reports the case as failed. A panic still runs
teardown. The runner rejects a requested harness if the selected checks have no case for it. Cursor’s opt-in check now
uses one genuine fresh launch and a separate chat that it seeds and reopens by ID. That extra seed turn is part of a
live Cursor run. Prompt-timed cases check that the suite captures the conversation ID before the hook reports the end of
that turn. The Claude trust check clears inherited workspace trust from its private copy and detects newly accepted
trust for the test workspace in the real file.

Ordinary brick-test cleanup can remove a prepared Mac pair that falls out of scope before its first test. If cleanup
finds only a saved test record, it still needs evidence that the native test setup passed its preflight. Tested pairs
keep their existing evidence and ownership checks.

### Open questions and possible follow-ups

The Cursor launch-prompt case still needs a live run with a working login. The existing TODO now tracks that
verification. There is no decision blocking this stack.

### Pull requests

- Require confirmed drift teardown and reject unmatched harness selections.
  [#5044](https://github.com/scode/farhelm/pull/5044/changes)
- Strengthen Cursor launch prompts, first-prompt capture ordering, and Claude workspace trust checks.
  [#5047](https://github.com/scode/farhelm/pull/5047/changes)
- Allow ordinary cleanup of a prepared Mac pair excluded by stop-after-P0 before testing.
  [#5048](https://github.com/scode/farhelm/pull/5048/changes)

### Checks

- Drift cleanup and runner. Recorded `cargo nextest run --locked -p farhelm --test e2e` with the offline drift group and
  `custom_agent_positional_report_corrects_to_the_parent_and_restarts_it` at `408c0b0a`. All 39 passed, including
  Restart through an owned test supervisor (run `928fb981-eba8-49f1-8e10-6b69371a513f`). Reused the 21 passing
  `python3 -B scripts/test-run-harness-drift.py` checks from `5f057e04` (run `6bfdb319-a7ba-422a-9736-4352a340f1bd`) and
  passing license and embedded-notice checks from `6b77a19f`. The runner and dependency inputs stayed unchanged.
- Launch and trust. The same nextest command selected the offline drift group at `dea6cc02`. All 42 passed, including
  Cursor’s selector, prompt timing, and Claude’s private configuration (run `7a2009dd-1be7-4ec9-94a3-78f2411cc27e`).
  Both Rust changes passed formatting, the test-sleep check, and integration Clippy.
- Brick coordinator. All 333 passed with `python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py' -v` at
  `7cfd4ee0` (run `f81b65f7-b00b-43fd-b2ce-5e16720e0d6b`). The final revision `406da567` kept the coordinator code and
  added an assertion isolating a saved tester record. Both focused tests for remote Mac cleanup passed with
  `test_native_coordinator.py` selected by discovery and `-k remote_mac` (run `d5082f3b-fa63-4adb-88c4-3327d25a1aed`).
  Markdown and whitespace checks passed. The instruction-link checker passed with one existing stale-citation warning in
  the archive.
- The Rust tests excluded opt-in vendor turns. Brick tests used modeled guests and synthetic evidence. No real Mac or
  release test was required. The changes have no GUI output to review.

### Review

Independent Sol source review passed each draft after corrections. The Cursor scope review confirmed that the fresh
launch, named seed, and named recall cover the two required launch behaviors. The executor compared the stack with
current main and found no changes to the affected behavior.
