## What this was about

The terminal text-size buttons read as “A−” and “A+” rather than recognizable actions. They now show magnifier-minus and
magnifier-plus icons, joined around the current size in points. The number is display-only and adds no keyboard stop.

## Things you should know

The size still applies to every open terminal and remains remembered on this device. The buttons, shortcuts, tooltips
and focus hand-back behave as before. The number updates after a step, reload or rebuilt session control. “Work in a
session” and both specifications describe the control; the unreleased toolbar changelog entry includes it.

The first toolbar check found that the extra readout pushed an action outside a 650px pane when the desktop tunnel pill
was present. The readout was narrowed to a fixed 22px slot; button targets and test assertions were retained. The exact
geometry case subsequently passed on both engines in separate runs.

## Open questions and possible follow-ups

No open product decisions. Native desktop operation was not exercised; WebKit and the existing native-tunnel
presentation fixture cover this change's engine and width risks. The unchanged WebKit geometry case ran close to its
60-second budget: one retry timed out during cleanup and the next passed in 58 seconds. A later upstream report recorded
the same pattern in another plan. The recurring case is recorded in FLAKES.md, TODO.md and the deflake exclusion list;
its cause remains unproven. No test assertions or deadlines changed.

## The PRs

- [#2806 — terminal zoom controls](https://github.com/scode/farhelm/pull/2806/changes), draft, based on main.

## Checks run, reused and skipped

- `cargo build -p farhelm` and `dx build --package farhelm-ui --platform web --release` passed with Dioxus 0.7.10. The
  web build was repeated for the final CSS correction and passed.
- `cargo clippy -p farhelm-ui --all-targets -- -D warnings` passed. `cargo nextest run -p farhelm-ui`, recorder
  `7794f718-6fe7-4465-b457-245a0fc48e7a`, passed all 805 tests with no skips. These cover the unchanged Rust source from
  the initial reviewed revision `b2ea5994e84c`; the final correction changes only CSS width and its comment.
- `terminal-tabs.spec.ts -g 'terminal text size'`, recorder `ee4bd844-6234-4257-ace1-6a1b2dc5f075`, passed six cases:
  three each on Chromium and WebKit. It covers buttons and shortcuts, terminal refits and pty dimensions, storage across
  reload, the actual rebuilt readout, fixed button positions across 9/10 points, and the Mac modifier guard. Ordinary
  retained screenshots from both engines were inspected.
- `header.spec.ts` and `tooltip-coverage.spec.ts`, recorder `cf332c82-504a-49b2-a7f6-66e37482045e`, ran 28 cases: 26
  passed and the native-tunnel 650px case failed on each engine before the width correction. The failure record is
  retained. The other 26 successes remain applicable: only the fixed readout width changed afterwards, with no changes
  to names, focus, menus or other actions.
- After the correction, the exact native-tunnel geometry case ran on both engines in recorder
  `1fc8687c-1044-4c63-83f9-dfddf8da2604`: Chromium passed, while WebKit reached the existing 60-second deadline during
  cleanup. A WebKit-only retry, recorder `9ebdc54c-8229-4787-a96b-19c2836e083e`, passed unchanged. That retry used
  generic recording, so its one-case console result has no strict validated browser-count claim. Both runs used the
  final reviewed source `a9b99e3932da`.
- The final readout/remount case, recorder `6229de33-9b95-4c64-b88e-b650610991f1`, passed on both engines at
  `a9b99e3932da`, confirming fixed button positions with the narrower readout. Both final screenshots were inspected and
  show the number and icons fitting cleanly.
- Runtime commands used the recorder, pinned tmux 3.7c, zero retries and the maintained concurrency policy. Reports and
  console capture are complete. Later passing checks do not erase the earlier width failures.
- After the final runtime checks, only the flake record and its matching TODO/exclusion entries were added. Their
  Markdown formatting passed; the recorded product-source coverage still applies.
- `cargo fmt --all -- --check`, changed-Markdown `dprint check`, `node --check` on the terminal asset, changelog format
  validation, and the test-sleep checker passed; the latter inspected 322 delays with none unexplained.
- No earlier session's runtime result was needed. The full Rust workspace, full browser suite, unrelated JS units,
  installer and provisioning tests were skipped because this localized control change leaves their behavior untouched.
  No JS unit test covers the changed font-size path; the browser tests execute the actual asset. No desktop build was
  needed for shared SVG markup/CSS. The incidental docs edit changes no link or heading, so no website build or preview
  was needed.

## Review gate outcome

The required fresh-context gpt-6.1-sol at high effort found no actionable issues in the implementation. It caught that a
proposed padding correction would be overridden in the native toolbar; that proposal was reverted before a test run. The
final fixed-width correction passed its follow-up review. The commit-message cold read passed.

### Landing

Landed as delivered; the only change at landing was resolving a TODO.md conflict with main (below). The terminal
text-size buttons in the session's top row now show magnifier-minus and magnifier-plus icons instead of "A−" and "A+",
with the current size in points between them. The number is display only and adds no keyboard stop; the buttons,
shortcuts, hover text and the shared, remembered size are unchanged.

- #2806: the zoom controls, their docs and specification text, an edit to the unreleased toolbar changelog entry,
  removal of its TODO entry, and the flake record described below.

#### What landed since the stack was based, and whether it interacted

The containerized-sessions plan (#2791 to #2794) landed after this plan was based. It changes terminal tabs' plumbing
inside the session view but not the top row's markup or styling, so nothing interacted. This round stacked this plan
with three others; two of them (menu-single-highlight, host-update-discoverability) were blocked at landing, answered by
the monitor and returned to their executors, and this plan and new-terminal-button were restacked directly on main and
checked again on their own (below). A fresh-context review of all four found the two that landed edit separate parts of
the top row and agree with each other. Two documentation PRs recording flakes (#2812, #2813) landed during this round,
appending TODO.md Deflake entries and deflake exclusion lines where this plan appends its own; the landing kept both
(see Checks).

#### Things you should know

- The top row now has two "+" glyphs: the new-terminal button's plain square "+" (new-terminal-button, landing right
  after this) and the magnifier "+" for larger text. The magnifier and the divider before the new-terminal button tell
  them apart; worth a look in visual review.
- The plan records a latent flake, "native tunnel preview keeps ports and actions inside a 650px pane" timing out near
  its 60-second budget on WebKit, in FLAKES.md, TODO.md and the deflake exclusion list. Every observation of it so far
  comes from branches that changed this toolbar (this plan and new-terminal-button), so whether it is an existing flake
  or a cost these changes added is unproven. At this round's combined tip it passed on both engines in the whole browser
  suite, and again in the checks after restacking.

#### Checks

The round first checked four plans stacked on main `229dddb6`: terminal-zoom-icons, new-terminal-button, and two that
the monitor then blocked at landing, answered on your behalf and returned to their executors (menu-single-highlight,
host-update-discoverability). On that combined stack:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint, the binaries' lint,
  the desktop-build checks, the check that every test delay states its reason, `dprint check`,
  `python3 releasing/check-changelog.py format`, the docs website build, the workspace debug build, the UI's release web
  build and `scripts/check-desktop-assets.sh`: all passed.
- The UI's Rust tests, run `1aa761f8-fe72-4097-b27a-05d4c474aea8`: all 819 passed; its desktop-build tests, run
  `1e0d5d05-d40a-44cd-98a6-cc4063efa941`: all 1002 passed. The Mac release test's coordinator tests: 214 passed. The
  UI's JavaScript tests, run `0a122921-4d83-4168-a49d-7e348510c07a`: 371 passed, 1 failed, caused by
  menu-single-highlight.
- `npx playwright test` in `e2e/`, the whole browser suite on Chromium and WebKit, three workers, no retries, run
  `10af6125-cdc5-4afc-bc9a-369377c00195`: 2792 passed, 14 skipped by design, 6 failed. The hover-text sweep on both
  engines was caused by host-update-discoverability. The known WebKit flake "restart with keeps gap focus and container
  Enter uses its enabled action" failed. On WebKit, "launcher gap keeps focus and container Enter explains an incomplete
  draft" failed once and passed on both engines in the rerun below. On Chromium, "plain Replace keeps a newer row's
  retry key after an older YOLO cancellation" failed, a recurrence of a flake FLAKES.md recorded as resolved on
  2026-10-15, and so did "terminal link ownership opens once and preserves program clicks in all mounted terminals",
  which has no flake record; both are rerun below. The 650px toolbar cases passed on both engines.
- The whole Rust workspace, run `43959a88-f565-461e-b3dd-760d8725cf4a`: 4546 passed, 41 skipped by design, 4 failed: two
  helm tests of the checkout configuration store's schema history, caused by host-update-discoverability, and the known
  flakes "shallow pause resumes without reset or replay" and "a hung save releases waiters at their deadline".

The two landing plans were then restacked directly on main `229dddb6` (terminal-zoom-icons `b29b61d1`,
new-terminal-button `6e551e29`), and on that stack:

- `dprint check`, `python3 releasing/check-changelog.py format`, the docs website build, `cargo fmt --all -- --check`,
  the web-target UI lint, the workspace debug build and the UI's release web build: all passed.
- The UI's Rust tests, run `bd1ce5ba-b6e3-4ff2-bfb1-156853ac63da`: all 813 passed. The UI's JavaScript tests, run
  `4676b2ac-9aac-4c4d-ba20-06cdcade4fb6`: all 365 passed.
- `npx playwright test` in `e2e/` with the terminal tabs, buttons, header, hover-text sweep, launcher Enter and Restart
  with specs, on Chromium and WebKit, run `548efb21-fc35-41ea-b1f2-a2e61fbd2ac5`: 281 passed, 1 failed: the known WebKit
  flake "restart with keeps gap focus and container Enter uses its enabled action" again.

- The two Chromium failures not covered above, on the same stack and both engines, five times each with no retries:
  "plain Replace keeps a newer row's retry key after an older YOLO cancellation", run
  `fc217f8a-e249-4b4b-be2b-59db1e523c90`, 10 of 10 passed; "terminal link ownership opens once and preserves program
  clicks in all mounted terminals", run `96630687-6ef3-42b3-84d8-6575d286dd44`, 10 of 10 passed. One failure each in the
  combined run is not recorded as a flake.

Main then moved: two documentation PRs recording flakes (#2812, #2813) landed, adding TODO.md Deflake entries and
deflake exclusion lines next to the ones terminal-zoom-icons adds. Rebasing onto that main conflicted only there; the
landing kept both sides (and restored one blank line between TODO entries), and the merged trees differ from the tested
ones only by main's additions in FLAKES.md, TODO.md, the deflake exclusion list and plan bookkeeping. No checks were
rerun after that rebase, because none of those files is read by a build or a test.

- Reused from the combined stack, for code the restack did not change: the whole Rust workspace and the whole browser
  suite (outside the specs rerun above), `cargo clippy --all-targets`, the binaries' lint, the desktop-build checks and
  their tests, the test-delay check, the Mac release test's coordinator tests and the desktop-asset check. Removing the
  two returned plans took code away and added none. Skipped: the desktop smoke tests (no desktop-specific code changed)
  and the native Mac app (not runnable here).

The report's checks were the executor's selected runs; this landing ran the whole browser suite and the whole Rust
workspace with this plan stacked under the others. Nothing in the executor's report became untrue at landing.
