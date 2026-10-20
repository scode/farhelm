# One Replace YOLO question per session

## What this was about

Replace from the sidebar and the top of a session could each leave a YOLO confirmation open for the same session.
Cancelling one left the other answerable. The maintainer approved sharing or excluding those questions within about 100
changed code, test and documentation lines, discarding the outcome if the fix exceeded that limit.

The fix excludes a second question for the same session and measures 92 changed lines, within the gate.

## Things you should know

The first question stays available when the other Replace control encounters the same YOLO refusal. Cancelling it leaves
no second question that can still be answered. Other sessions can continue independently. Questions keep the session
snapshot they originally covered, and cancelling an older question preserves a newer retry attempt.

Answering, cancelling, replacing a sidebar question, or navigating away from a header question releases its ownership. A
failed "don't ask again" write retains or restores the question with its explanation. Review caught one case where
rejecting a duplicate could clear another session's explanation; that was corrected before the final browser run. That
specific error-preservation case was checked by code review; the browser run covers refusal admission and successful
permanent answers.

## Open questions and possible follow-ups

None required for this outcome.

## The PRs

- [#2410](https://github.com/scode/farhelm/pull/2410/changes): prevent two Replace YOLO questions for one session; open
  draft.

## Checks run, reused and skipped

- Recorded
  `cargo nextest run -p farhelm-ui --lib -E 'test(list::view::tests::a_cancelled_yolo_replace_question_dispatches_nothing) | test(ops::tests::)'`
  passed 14 tests, with 535 filtered out and no runtime substrate skips, run `e3336e27-b395-4c56-a7d0-cb11d1ece020`.
  Pinned nextest and tmux, four slots and zero retries were used. This covered the initial 77-line working patch on base
  `596f1b7e`; its cancellation and claim-helper coverage is reused after the correction because those tested helpers and
  tests did not change. The correction moves an error update inside successful question admission; the final web build
  and browser run cover that opening path.
- Recorded
  `npx playwright test 'replace\.spec\.ts|yolo-guard\.spec\.ts' -g 'keeps a newer same-source key after old|a (header|sidebar) replace keeps its confirmed nothing-alive answer'`
  passed all 12 cases, six per engine, run `4ea789b9-dba7-4c38-a939-939b69a974c1`, on tested revision `896e4b4c`.
  Chromium and WebKit used one worker, zero retries and pinned tmux. The new cases exercise both question-opening
  orders, one remaining question after the second refusal, cancellation, preservation of a newer retry, and navigation
  cleanup. The existing cases retain the confirmed deletion precondition through one-time and permanent YOLO answers.
  Source, lifecycle and result evidence is complete; no failures or skips occurred.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, changed Markdown dprint checks,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` and the isolated test-sleep
  checker passed for the corrected change. The sleep checker inspected 304 delays with no missing rationale. The final
  release web build passed after the review correction.
- The cached native backend build from `7355efcf` is reused: the complete path inventory through base `596f1b7e`
  contains only TODO and plan bookkeeping, so no backend build input changed. New main changes through `5bade3e6` were
  read in full and contain only claims and other delivery reports. No product, specification or governing instruction
  interaction required a rebase. Commit creation and the PR-link amendment leave the tested code and fixtures unchanged
  at final PR head `9b47bbe0`.
- Broader Rust and browser suites, native desktop runtime, installer and provisioning tests were skipped. Focused
  claim/cancellation units and both-engine Replace tests cover the changed confirmation ownership and lifetimes; no
  backend, platform shell, installer or provisioning behavior changed. CI was not dispatched; PRs do not start it.

## The review gate's outcome

One fresh-context gpt-6.1-sol reviewer at high effort found the question-error preservation issue described above,
verified its correction and reported no remaining findings. The same reviewer independently confirmed the final 92-line
size gate. Fresh cold readers checked commit and PR wording; jargon in the first draft was removed. The outcome ledger
names the change, bookmark and PR, its feedback item is removed, and a fixed changelog fragment is included.
