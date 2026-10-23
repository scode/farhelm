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

### Landing

Landed as #2603, #2606, #2607, #2608, #2611, #2613, #2615 and #2618, merged bottom-up, with the same content the report
describes. It was the first of two plans in this landing round, which stacked both and checked them together before
either merged; duplicate-uninstall-retry-advice followed it.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained bind-gui-answers (GUI confirmations, YOLO questions, quick-launch sequences and
other prompts now act on what the user saw when answering), and documentation and planning files. This plan's browser
test changes were written before bind-gui-answers, so a fresh-context reviewer and I checked that their premises still
hold on the landed UI:

- bind-gui-answers' only edit to the quick-launch spec is a separate new test; this plan's edit to another test in that
  file composes with it, and pressing the quick-launch shortcut again while the New form is open is still accepted under
  the new rules. The create request still carries a freshly made retry key when the template changes.
- The custom-permissions, clone, notifications and Settings specs use controls and settings that bind-gui-answers did
  not change, and the cleanup orders this plan introduces are sound.
- The Rust test changes (the approval poll's deadline now bounds the whole request; a background child is owned before
  readiness waits) touch files nothing else changed.

#### Fixes made during the landing

None to the plan's changes. The review-queue and triage records conflicted with main's new entries; the landing kept
main's entries and this plan's completed Execution lines.

#### Possible follow-ups

- Found at landing, low severity: the start-stack cleanup script's new check that the stand-in supervisor is alive can
  pass during the brief moment the stand-in is still a shell about to replace itself with `sleep`, since that shell's
  command line already matches (as the stack's own cleanup's search would). It would only matter if that last step
  failed (for example, `sleep` missing), which does not happen in practice; the cost would be the test passing without
  proving cleanup, the gap #2613 set out to close. Possible follow-up: also require the matched command to start with
  `farhelm`.

#### Checks

The two plans change about 200 lines: tests and fixtures in Rust, five browser specs and the start-stack cleanup script,
plus the helm's uninstall refusal text. This round ran every test they change, and the rest of the helm crate, on both
plans stacked together on this round's main (which already held bind-gui-answers), including the triage-record conflict
resolution:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, the isolated test-sleep check (tests changed), `dprint check` on the
  changed Markdown files, `python3 releasing/check-changelog.py format`, `shellcheck --severity=warning` on the changed
  script, the workspace debug build and the UI's release web build the browser tests drive: all passed. Shellcheck's
  informational notes on that script predate these plans.
- `cargo nextest run -p farhelm-helm -p farhelm` with the filter
  `package(farhelm-helm) | binary(spare_builder) | test(agent_listing_real_stack::)`, through the test recorder with
  pinned nextest and tmux, four slots, no retries, run `e468ec4f-d6fb-428f-8478-cec9a9249547`: 1307 passed, none failed,
  with no runtime `SKIPPED` message in the retained output, so the end-to-end tests ran for real. This covers the whole
  helm crate (the uninstall refusal), the spare-builder tests and the agent-listing end-to-end tests this round changed.
- `bash scripts/test-start-stack-cleanup.sh` through the recorder, run `f458427a-72d1-4cd8-b7dd-a867843b5e41`: every
  case reported ok.
- `npx playwright test` in `e2e/` for `clone`, `custom-permissions`, `notifications`, `quick-launch` and `settings` on
  Chromium and WebKit, three workers each with its own helm and supervisors, no retries, run
  `44d59b3c-d844-46f4-8645-cdcdcf3a7ae6`: 192 passed, none failed. The "owned fixture cleanup failed" errors in its
  output come from a notification test that injects cleanup failures on purpose and passed.
- Reused: none. Skipped: the rest of the Rust workspace and browser suite (only the tests above and the helm's refusal
  text changed), the UI, desktop and JavaScript tests (no UI or asset code changed), the website build, the doctests and
  the installer (unchanged).

The report's checks ran each changed test on its base; this round ran them again on both plans stacked on the landed UI.
Nothing in the executor's report became untrue at landing.
