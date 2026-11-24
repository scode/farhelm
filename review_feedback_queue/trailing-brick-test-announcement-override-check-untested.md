# The brick test's announcement override check has no negative test

Reviewed commit: 356718b9d0bca3a27758ec6904688da2f54b9639

## TLDR

The brick test's guard that the signed-announcement launch really used the test feed (and that the follow-up relaunch
really dropped it) could silently stop working and its unit tests would still pass. A broken guard would let the P2
announcements check pass on a launch that never used the standing test feed override, or fail to notice the override
leaking into the normal relaunch.

## Details

**other.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it.

### Reviewer record (`g10-brick-announcements/f1`)

Location: `releasing/mac-vm-test/test_guest_operations.py:808`. Summary: Announcement override check has no negative
test; its asserted result is the expectation, not the observation

Reviewed commit: 356718b9d0bca3a27758ec6904688da2f54b9639 (still present at tip
e159ffc79ab147be44e92a92b59571650a4f322d).

TLDR: The brick test's guard that the signed-announcement launch really used the test feed (and that the follow-up
relaunch really dropped it) could silently stop working and its unit tests would still pass. A broken guard would let
the P2 announcements check pass on a launch that never used the standing test feed override, or fail to notice the
override leaking into the normal relaunch.

Details: `check_announcement_launch` (releasing/mac-vm-test/guest_operations.py:571-585) reads the candidate's launch
environment with `ps -Eww` and raises when FARHELM_ANNOUNCEMENTS_PATH is not exactly `announcements-test.json` on the
staged launch, or is present at all on the relaunch. It returns `expected` (derived from the `staged` flag), not the
matched value. The only test, `test_announcement_override_is_scoped_to_one_candidate_launch`
(releasing/mac-vm-test/test_guest_operations.py:~808-840), feeds ps output that always agrees with the expectation and
then asserts `staged['announcement_override'] == 'announcements-test.json'` and
`normal['announcement_override'] is None`. Those assertions are tautological: they hold for any implementation that
returns the expectation without comparing (e.g. a regex typo that never matches combined with a comparison that no
longer raises, or the comparison removed). No test feeds a staged launch whose environment lacks the variable (or
carries a different value), or a relaunch whose environment still carries it, and asserts the RuntimeError / latched
failure. Verify by reading the test: the ps stub appends the variable exactly when the preceding `open` argv had
`--env`, so the mismatch branch at guest_operations.py:584-585 is never exercised. Fix: add subtests where the stub
returns the wrong environment for each launch and assert `dispatch` raises 'expected announcement override' and
`operations.failure` is latched; optionally return the observed value (`match[1] if match else None`) so the receipt
records the observation.

Suggested bucket: other — a test-quality gap in maintainer release tooling with no shown user-facing failure.

Confidence: confirmed (source shows the missing negative case and the tautological assertions).

proposed_drop: none

possible_cover: none

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g10-brick-announcements/f1`.
