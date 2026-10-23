# Eight test-only hygiene fixes from the 2026-10-21 auto-triage

## What this was about

The 2026-10-21 auto-triage found eight test problems that you had asked to be decided without asking when they were
clearly bugs with a straightforward fix. None of them was a product bug. Four tests armed their cleanup too late, so a
failed setup step could leave a session, a background process or a changed setting behind for the tests that run after
them against the same test helm and supervisor. Three tests could pass without proving what they claim, and one test
helper ignored its own deadline while waiting on a stalled reply. Each fix had a limit of about 100 changed lines; all
eight came in well under it (the largest is 41 lines). No product code changed.

## Things you should know

- **Approval waits** ([#2603](https://github.com/scode/farhelm/pull/2603/changes)): the end-to-end test helper that
  waits for an agent's approval card to appear on the helm now bounds each request by its deadline, so a helm that
  stalls mid-reply fails with the helper's own message instead of hanging until the test runner kills it. The review
  caught that the first version would have reported a stall for the much more common case of a healthy helm that never
  shows a card; the deadline is now checked first, as the sibling readiness probe does, so each case gets its own
  message.
- **Retry key** ([#2606](https://github.com/scode/farhelm/pull/2606/changes)): when a quick-launch template changes
  between two launches, the test now requires the second launch to carry a real retry key (the key that lets the helm
  recognise a retried launch, so a retry does not start a second session), not merely a different one. A launch that
  sent no key at all used to pass; the app sends one today, so this only closes the test's gap.
- **Cleanup order** ([#2607](https://github.com/scode/farhelm/pull/2607/changes),
  [#2611](https://github.com/scode/farhelm/pull/2611/changes),
  [#2618](https://github.com/scode/farhelm/pull/2618/changes)): the clone test's session, the two notification tests'
  sessions, and the background process the prepared-checkout builder tests start are each owned by cleanup as soon as
  they exist.
- **Custom model acceptance** ([#2608](https://github.com/scode/farhelm/pull/2608/changes)): the test now uses a
  different model name in each round, checks the model field opened, and after Enter checks it closed showing that name.
  Only a closed field shows the accepted model, so this proves Enter accepted it; before, the test could pass if Enter
  did nothing.
- **Stack cleanup script** ([#2613](https://github.com/scode/farhelm/pull/2613/changes)): the test script for the
  browser-test stack's cleanup checks that stopping the stack also stops a supervisor that replaced the stack's own. It
  now confirms its stand-in supervisor is really running, in the form the stack's cleanup looks for, before it stops the
  stack, and fails otherwise; before, a stand-in that had already died let the check pass.
- **Prepared-checkout setting** ([#2615](https://github.com/scode/farhelm/pull/2615/changes)): the settings test now
  waits for every save the page sent, not only the one it held, before restoring the "keep a checkout ready" setting,
  which every test on that helm shares.

## Open questions and possible follow-ups

None that need a decision. Two small leftovers remain, both narrower than what the outcomes asked for: a save the
settings page sends after the test's cleanup has started waiting can still race the restore (the earlier fix to the
checkout-folder settings test, #2154, has the same gap), and in the prepared-checkout builder tests a timeout while
waiting for the background process's own pid can still leave it running (that window is a single file write). I
recommend leaving both: each needs a test to fail at one exact instant, and no such failure has been seen.

The plan suggested optional fault injections (breaking the behavior on purpose to watch the stronger test fail) for
#2606, #2608 and #2613; I did not run them. Each review reasoned through the failing case instead, and #2608's review
found and closed the one way its check could still pass without proof.

## The PRs

- [#2603](https://github.com/scode/farhelm/pull/2603/changes): bound each approval poll by its deadline.
- [#2606](https://github.com/scode/farhelm/pull/2606/changes): require a retry key when a changed template relaunches.
- [#2607](https://github.com/scode/farhelm/pull/2607/changes): own the held-reveal session from the moment it exists.
- [#2608](https://github.com/scode/farhelm/pull/2608/changes): prove the custom model is accepted before checking
  effort.
- [#2611](https://github.com/scode/farhelm/pull/2611/changes): clean up each notification test session from its create.
- [#2613](https://github.com/scode/farhelm/pull/2613/changes): check the stand-in supervisor is alive before signalling.
- [#2615](https://github.com/scode/farhelm/pull/2615/changes): settle every prepared-checkout save before restoring it.
- [#2618](https://github.com/scode/farhelm/pull/2618/changes): own the background child before spare-builder readiness
  waits.

Each is based on the one before it.

## Checks run, reused and skipped

- One `cargo build` and one release web build at the stack's base served every PR (no product code changes).
- #2603: recorded `cargo nextest run -p farhelm --test e2e` selecting the helper's two callers, run
  `fd56935c-d793-4aa4-8e3c-8e7924ff3bc5`: both passed, no runtime skips (pinned nextest and tmux).
- #2606: recorded `npx playwright test 'quick-launch\.spec\.ts'` on Chromium and WebKit, run
  `099cc48d-2da1-4fcd-93f8-6891f0f51fd4`: 86 passed.
- #2607: recorded `npx playwright test 'clone\.spec\.ts'` on Chromium and WebKit, run
  `6b36451f-f7f7-428d-a64e-cf4f8192932f`: 26 passed.
- #2608: recorded `npx playwright test 'custom-permissions\.spec\.ts'` on both engines, run
  `af14d270-4ecd-4ea9-aea7-51e0ed961f74`: 20 passed (after the review's added check that the model field opened).
- #2611: recorded `npx playwright test 'notifications\.spec\.ts'` on both engines, run
  `d3597825-18aa-4e0a-9d7b-24efd4c5bc1b`: 22 passed.
- #2613: recorded `bash scripts/test-start-stack-cleanup.sh`, run `9889141a-6244-45f5-a4af-81289504e57e`: all seven
  phases passed, including both replacement phases (its parent-SIGTERM phase, an open flake in FLAKES.md, passed too).
  The review's two edits afterwards changed only the failure message and a comment. `shellcheck` reports only the three
  notices already on main.
- #2615: recorded `npx playwright test 'settings\.spec\.ts'` on both engines, run
  `8842dec7-1cdf-413d-8f22-ff0c98394bdd`: 42 passed; a comment-only edit followed.
- #2618: recorded `cargo nextest run -p farhelm --test spare_builder` selecting the two tests (pinned nextest and tmux),
  run `2d23b043-39f0-4aa3-b29a-286fd0294b2e`: both passed.
- On every Rust or browser test PR: the isolated test-sleep checker (314 delays, none missing a rationale), and
  `cargo clippy -p farhelm --tests -- -D warnings` and `cargo fmt --all -- --check` for the Rust ones; `dprint check` on
  the changed Markdown.
- Skipped: the optional fault injections (see above) and every wider suite, since only these tests changed. Main moved
  during the run by TODO, planning and instruction edits that touch none of these files, so no rebase was needed. CI was
  not dispatched; it does not run on PRs.

## The review gate's outcome

Each PR got a fresh-context Opus 5.5 review at high effort with the plan, its triage entry and the test-authoring
checklist, and every commit message went through a fresh cold reader. No review found a blocking problem. The
substantive catches, all fixed: #2603's misleading failure message described above; #2608's missing check that the field
had opened before Enter, without which a field that never opened could still pass; #2613's failure message, which now
says what the stand-in's process actually looked like; and wording and bookkeeping corrections elsewhere. All eight PRs
cleared the review gate.
