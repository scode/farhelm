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

### Landing

Landed as #2112, last of the round's nine plans, tested stacked with the others on main.

#### Review

An independent fresh-context review found nothing blocking. The evidence-hash check moved into a shared helper with the
same exceptions and limits; cleanup now counts a failed or unavailable feedback check as retained only when its recorded
hashes still match, refusing before any guest is touched otherwise. Every recorded result has carried hashes since
before this check existed, and the test fails without the fix.

#### Checks

Run on all nine of this round's plans stacked on main (terminal-link-safety, ntfy-token-server-change,
remote-uninstall-fixes, update-failure-reporting, docs-accuracy-fixes, host-picker-dotted-i,
setup-advice-state-directory, copy-guidance-resize, brick-test-feedback-evidence), with every landing fix in:

- `cargo fmt --all -- --check`, `dprint check`, the changelog lint and the sleep checker (301 delays, none missing a
  reason); `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, the UI crate's
  clippy with the desktop feature and `cargo check -p farhelm-desktop`; the brick-test tooling's own test.
- Through the recorder: the protocol and helm packages in full with pinned tmux, run
  `6352fad3-a3b1-48dc-9884-3efc17ee03c6`: 1258 passed; the CLI's own unit tests, run
  `0e1855d7-6f43-4da7-8a9a-495936ae565e`: 171 passed; the UI crate with the desktop feature, run
  `513addbb-2505-4c54-bc07-2ac2ec06713b`: 655 passed.
- The JS harness (257 passed) and the website build.
- The terminal-link-clicks, terminal-links, terminal-files, ntfy-settings, provisioning, settings and tooltip-coverage
  specs on Chromium and WebKit, run `97a26936-2b7b-42a5-af59-8f0fdb0b9d78`: 208 passed, 2 failed. Both failures were the
  clipped-disclosure test pinning the refusal's old layering after the quick switcher closed (see terminal-link-safety).
  After that expectation was fixed, the terminal-link-clicks spec on both engines, run
  `a698ea79-451d-4dd4-87f5-24ef1d4b1cad`: 16 passed. An intermediate run, `6113fd52-60bf-4730-ae85-5e9dc2e90d46`, failed
  on WebKit only because the first corrected expectation assumed Chromium's behavior.
- Not run: the supervisor package and the CLI's end-to-end targets, which no PR in this round changes; other browser
  specs.
