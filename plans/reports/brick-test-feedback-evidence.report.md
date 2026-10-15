## What this was about

The release brick test keeps its final Mac and Linux test guests until the feedback check is recorded. Cleanup checked
the saved evidence for a successful feedback check, but after a failed or unavailable check it could delete both guests
even if the exported screenshot had disappeared or changed. If the guest still held the only usable copy, deleting it
could lose the failure evidence. The missing cleanup gate was confirmed; loss of a real guest's evidence was not.

The maintainer delegated a small fix confined to the coordinator's cleanup, a shared digest check and its regression
tests. Cleanup now verifies every recorded feedback evidence digest before deleting guests, regardless of the feedback
result. Missing or changed evidence refuses cleanup before any guest operation. Restoring the accepted bytes permits
cleanup again.

## Things you should know

A failed or unavailable feedback check still allows ordinary cleanup when its evidence is intact. P0 and P1 verdict
rules, the explicit failed-run discard path and the guest tester protocol are unchanged. A late request to stop after P0
does not bypass retention for a feedback observation already recorded.

This is release-test tooling; the shipped app is unchanged. No live guests were used and no feedback was sent. The tests
use the existing synthetic Tart substrate.

## Open questions and possible follow-ups

None. The fix met the recorded complexity gate without a scope departure.

## The PRs

- [#2112 — retain failed feedback evidence before guest cleanup](https://github.com/scode/farhelm/pull/2112/changes),
  draft, based on main.

## Checks run, reused and skipped

- Ran `python3 releasing/mac-vm-test/test_brick_test.py`: all 40 coordinator tests passed. The lost-evidence regression
  covers passing, failed and unavailable feedback with changed exports, missing exports and a late P0-only stop. It
  asserts a passing P0 verdict, refusal before any Tart call, live guests until recovery, and cleanup after restoring
  the exact bytes. The existing intact-failure test still proves failed feedback permits cleanup and bonus work.
- Ran `dprint check TRIAGE_OUTCOMES.md review_feedback_queue/INDEX.md` and the Git diff whitespace check: passed.
- Reused the full coordinator result after adding the PR link; no executable source or test changed after that run.
- Skipped Rust, browser, installer and live Mac release tests because this change only affects the Python coordinator's
  evidence gate, which the synthetic suite directly exercises. No changelog fragment applies to this tooling change.

## Review gate outcome

The required fresh-context gpt-6.1-sol high source review reported no findings. It received the complete outcome,
complexity gate and test-authoring contract, and separately passed both focused cleanup regressions. The commit and PR
wording passed its independent cold read. A separate report cold read checks this report before delivery. Implementation
and investigation stayed with the executor; no workhorse was used.
