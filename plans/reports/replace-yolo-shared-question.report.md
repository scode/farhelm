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

### Landing

Landed as #2410. It was the eighth of sixteen plans in this landing round, after launcher-retry-identity,
launcher-composer-drafts, terminal-link-and-copy-fixes, transfer-leftover-cleanup, trash-unknown-totals,
reachable-at-small-sizes and app-dialog-focus. Its content is what the report describes, rebased onto this round's main,
with one browser test fix described below.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained round 107's four plans, the seven plans earlier in this round, and documentation and
plan bookkeeping. A fresh-context reviewer and I read them and the other plans of this round against this plan. Two
touch the same ground:

- host-entry-session-ids, from round 107, changed how sessions are identified. This plan keeps one YOLO question per
  session by the session's identity; the sidebar and the session header use the same full identity after that change, so
  a question raised by one still excludes a second from the other. Its new browser check needed one fix for the new
  identity (below).
- launcher-retry-identity, earlier in this round, keeps a Replace retry on the same attempt until it succeeds or the
  helm proves nothing was created. When this plan refuses a second YOLO question, the session header shows it as an
  ordinary Replace error, and from the sidebar the existing question simply stays open. Either way the next retry reuses
  the same attempt, which is what launcher-retry-identity expects; this plan's own check that cancelling an older
  question keeps a newer retry attempt passed on the combined stack.

The only merge conflicts were in the review feedback queue's index, where each plan removes its own entries; all
removals are kept.

#### Fixes made during the landing

- The new browser check, in which plain Replace keeps a newer retry attempt after an older YOLO question is cancelled,
  waited for the refused Replace response by matching the session's ID in the request address. After
  host-entry-session-ids, that ID is a full reference whose colon the UI encodes in the address, so the wait never
  matched and the test would have timed out in both the sidebar and header variants. It now matches the encoded ID, as
  the same test's interception of that request already did (#2410). The full browser run below, under Checks, ran with
  this fix and passed the test on both engines.

#### Checks

Every plan in this round was executed on a main from before round 107's session-identity change, so none had been tested
against it. That is why this round ran the whole Rust workspace and the full browser suite rather than a selection. All
of the following ran now, on all sixteen plans of this round stacked together on main, including the fixes made while
landing plans in this round:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on every changed Markdown, JSON and TOML file,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` and its `--self-test`,
  `python3 releasing/notes-review-page.py --self-test`, `scripts/check-desktop-assets.sh`, and the website build
  (`bun run build` in `website/`): all passed.
- `cargo nextest run --workspace --exclude farhelm-desktop` through the test recorder with pinned nextest and tmux, four
  parallel test slots and no retries, run `40f2b98b-b160-4239-a653-44cd0bcdcf96`: 3948 passed.
  `cargo nextest run -p farhelm-ui --features desktop`, run `d288b68b-249f-4f21-9fe0-d4d6d1c4a738`: 717 passed.
  `node --test` in `crates/farhelm-ui/js-tests`, run `f82f83f8-67c9-4e07-99ed-467c9830dc9f`: 317 passed.
- The full browser suite (`npx playwright test` in `e2e/`) on Chromium and WebKit, one worker, no retries, run
  `81d76a96-912f-47ed-8e1f-57423edbeb32` (kept): 2183 passed and 14 skipped. Of the skips, 9 are the WebKit clipboard
  cases, because Playwright cannot grant WebKit clipboard permissions; 4 run only against a real, signed-in agent; and 1
  is a pixel capture that runs only on Chromium. One test failed: on WebKit, "model-only number key opens New with a
  temporary composition explanation" in `templates.spec.ts` timed out at its first step, waiting 60 seconds for the
  header's Templates button to settle before clicking it. No plan in this round changes that button or that test, and
  the file's other WebKit tests that open the same dialog passed.
  `npx playwright test --project=webkit-templates -g 'model-only number key opens New with a temporary composition explanation' --repeat-each=20`
  then passed all 20 (run `6c382916-77a1-4ecb-b4ee-a1dae8cac848`). The cause is probably machine load during the full
  run, not something this round broke, but that is not confirmed; on one observation it is not recorded as a flake.
- Skipped: the native Mac app (not runnable here) and the installer (unchanged).

The report was written on a main from before round 107's session-identity change, so its statement that no rebase was
needed and its focused checks no longer describe what landed; the combined runs above supersede them. Nothing else in
the executor's report became untrue at landing.
