## What this was about

The control for opening a terminal looked too much like another terminal tab. It is now a square **+** after a thin
divider, with the accessible name **New terminal** and a tooltip explaining that it opens a shell in the session's
directory. Its action, neutral button styling and unavailable states are unchanged.

## Things you should know

The divider is decoration outside the button's click target. The brick-test instructions describe the new-terminal
button (+) after the tabs, so the same wording works with the previous labelled button and the new icon. The **Work in a
session** docs page identifies the control in prose and alt text. Its published tab-strip screenshot still shows the
previous control; this plan deliberately did not recapture or publish screenshots.

One existing WebKit toolbar-layout case has little time left under its 60-second deadline: it timed out twice, then
passed unchanged in 58.7 seconds. No geometry assertion failure was established. The timing cause remains unproven, and
neither the original selected browser run nor the reproduction batch was green. All selected cases have an actual
passing observation, with the failed runs retained separately. No test deadline or product behavior was changed to
obtain that pass.

## Open questions and possible follow-ups

Refresh the tab-strip docs screenshot in a separately requested screenshot refresh. The WebKit layout case's timing
margin may warrant investigation if it recurs; this run does not establish a product regression or a latent-flake cause.

## The PRs

- [#2805: distinguish the new-terminal button from tabs](https://github.com/scode/farhelm/pull/2805/changes), draft,
  head `477928c9aa94e08ce4f01bf9e63ea2fab937cb3a`.

## Checks run, reused and skipped

Run now:

- `cargo fmt --all -- --check`, `cargo clippy -p farhelm-ui --all-targets -- -D warnings`, scoped `dprint check`,
  `git diff --check`, `python3 releasing/check-changelog.py format`, and `python3 scripts/check-agents-md.py` passed.
  The instruction checker had zero errors and three existing citation warnings. The isolated interpreter ran
  `python -B scripts/check-test-sleeps.py`: 322 delays inspected, zero unexplained.
- Recorder `cb9d2cc7-8bf2-4731-aabf-b8ec1a84ce7a`:
  `python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py' -v`, 214 portable coordinator tests passed.
  These use fictional fixtures, not actual VMs or release checks.
- Recorder `508e9e72-980c-4fbe-b837-4a52665e7b70`: `cd website && bun install --frozen-lockfile && bun run build`
  passed, 32 pages and all internal links valid, using Node 26.12.1 and Bun 1.4.2. The preceding two recorded commands
  tested the complete dirty change above main `bef4fc05ea1ec602fb97135264656f4fba043a75`, fingerprint
  `cb7b3521557e7a02481ef8a0df436a5ecffbdb45bb6591da6e3751755a440609`; those exact bytes became the PR head.
- `cargo build -p farhelm -p farhelm-fixtures` and Dioxus 0.7.10's
  `dx build --package farhelm-ui --platform web --release` prepared the browser stack. Two initial web-build invocations
  selected an unrelated `dx` executable and failed before compiling; selecting the Dioxus executable corrected the
  invocation.
- Recorder `19b344e5-b3b4-4f45-ae92-512a9d9fcc21`, Playwright mode, ran the following selection from `e2e`, with
  Chromium and WebKit, three workers, zero retries, Playwright 1.64.0 and pinned tmux 3.7c:

  ```sh
  npx playwright test 'tests/(buttons|header|tooltip-coverage|terminal-tabs)\.spec\.ts' -g 'normal, neutral|native tunnel preview|tabs and actions share|header actions stay ordered|the session view gives every|a tab runs a real shell|the strip labels tabs|repeated activations while an open|an open the supervisor refuses|closing a tab confirms'
  ```

  Result: Chromium 10 passed; WebKit nine passed and the native-tunnel toolbar geometry case timed out. No skips,
  unstarted cases or interrupted cases. The successful cases cover button tiers and the new accessible name, shared
  toolbar geometry and action order, tooltip coverage, and tab shell, labels, close, repeated-open and refusal behavior.
- Recorder `13f31fe8-cb80-4a0d-85f8-507cf557baf3` was a failed reproduction invocation: an anchored grep matched no
  tests. It is not passing evidence.
- Recorder `733f7113-5cfc-4c70-854d-193ced1a9c97`, generic repetition mode, ran only the failing case twice
  sequentially:

  ```sh
  npx playwright test --project=webkit-header -g 'native tunnel preview keeps ports and actions inside a 650px pane' --repeat-each=2 --workers=1
  ```

  First attempt timed out; second passed in 58.7 seconds at the unchanged 60-second deadline. The batch exited nonzero.
  Both browser selections used unchanged PR head `477928c9aa94e08ce4f01bf9e63ea2fab937cb3a`, fingerprint
  `a69ffd16370e233413066c96ae369e756d7946b195ae7a8840e0ce0ce90cf9c6`, on Linux x86_64 with Python 3.15.2 and Node
  26.12.1. Retained output and the failed trace were inspected. Records have complete source identity, stream EOF, zero
  output omissions and clean process-group cleanup; generic reproduction counts come from its retained runner output.
  Failed evidence and attachments remain private and retained.

Reused: the existing browser builds for the exact-case reproduction, because no source changed between runs. Main still
matched the PR's base at the final interaction assessment, so no rebase or repeated runtime checks were warranted.

Skipped: the full Rust and browser batteries, unrelated JavaScript tests, desktop/installer/provisioning runtime, and
actual Mac VM tests. This is a markup/CSS change with unchanged action and state logic; lint compilation and the focused
browser observations cover its implementation risks. Native app appearance and screenshot freshness are not claimed. No
preview server was started for this unattended incidental docs edit.

## Review gate outcome

The required fresh-context gpt-6.1-sol reviewer at high effort reviewed the complete PR adversarially against this plan,
including the full test-authoring checklist, and reported no actionable findings. A separate cold reader checked the
commit and PR wording. The documentation pass covered every touched file. The source TODO entry was removed, and the PR
remains a draft for the landing monitor.

### Landing

Landed as delivered, with no fixes at landing. The control for opening a terminal no longer looks like another tab: it
is a square "+" after a thin divider, named "New terminal", with hover text saying it opens a shell in the session's
directory. What it does and when it is unavailable are unchanged. The Mac release test's instructions now describe it as
the new-terminal button (+) after the tabs, which also fits releases cut before this change.

- #2805: the button, its docs and specification text, the release test's wording, a changelog entry, and removal of its
  TODO entry.

#### What landed since the stack was based, and whether it interacted

The containerized-sessions plan (#2791 to #2794) and terminal-zoom-icons (landing right before this) landed after this
plan was based. Neither touches the tab strip's add control, and the review found the two top-row changes consistent;
each removes only its own TODO entry. Two other plans from this round were blocked at landing, answered by the monitor
and returned to their executors, and this plan was checked again on top of terminal-zoom-icons alone (below). Two
documentation PRs recording flakes (#2812, #2813) landed during this round; they touch only flake records and merged
without touching this plan's changes.

#### Things you should know

- The "Work in a session" docs page's tab-strip screenshot still shows the old labelled button; the plan deliberately
  left recapturing it to a separately requested screenshot refresh.
- The top row has two "+" glyphs now, the new-terminal button's and the larger-text magnifier's (see
  terminal-zoom-icons' notes).
- This plan's executor hit the WebKit timeout of "native tunnel preview keeps ports and actions inside a 650px pane"
  twice before it passed; terminal-zoom-icons' notes cover that flake record.
- The release test's coordinator tests (214) passed; no actual Mac VM run was needed or done.

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
workspace with this plan stacked with the others. Nothing in the executor's report became untrue at landing.
