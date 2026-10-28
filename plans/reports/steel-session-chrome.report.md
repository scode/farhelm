## What this was about

The open session's toolbar spent space on separate Restart with and Replace with buttons, while desktop tunnels used
another row above the terminal. This plan puts those variants in split-button menus and moves tunnel controls beside the
terminal tabs, following the approved Steel design.

## Things you should know

The toolbar keeps its existing confirmations and session-operation guards. Text-size controls stay visible when tabs
scroll. Restart with remains available independently when a saved resume command permits naming a conversation before
ordinary Restart becomes available; unavailable actions keep their full explanations.

The toolbar was photographed in both themes, wide and at an actual 650px main pane. The independently available
Restart-with case passed integration tests in both browser engines but was not separately photographed.

Desktop tunnels show one mapping or a count beside the tabs. The list closes individual tunnels or attempts the
displayed list in order with Close all. Successful closes disappear; failures remain visible through refresh and can be
dismissed. List and close access remain available while the host is disconnected, and there is no permanent tunnel row.
Action spacing is tighter while tunnels exist, including on wide windows. This deliberately keeps enough space for
readable tabs, both ports and all actions at 650px; labels and button height stay the same.

The tunnel images use the plan's permitted fallback: desktop controls reproduced in the ordinary staged browser capture,
with the desktop layout forced. They establish appearance and layout. Rust tests exercise serial Close all progress,
retained errors and the existing tunnel permission prompt's lifetime. Source review covers desktop button handlers,
tunnel-close request wiring and keyboard focus handoffs; no actual native window interaction or macOS title-band pixels
were observed.

## Open questions and possible follow-ups

The later sentence-case plan owns the older shared tooltip wording. The toolbar reuses the foundation's neutral button
tier and keeps full explanations even where menus are taller than the approximate reference mockup.

## The PRs

- [#2753: session Restart and Replace menus](https://github.com/scode/farhelm/pull/2753/changes), draft.
- [#2754: session tunnel pill and close controls](https://github.com/scode/farhelm/pull/2754/changes), draft, above
  #2753.

## Checks run, reused and skipped

Runtime commands below ran through `scripts/record-test-run.py` on Linux. Browser commands ran from `e2e/` on Chromium
and WebKit, with three workers and no retries. Nextest used four slots, no retries and the repository's pinned runner;
tmux-dependent checks used the repository-built pinned tmux. JavaScript commands ran from `crates/farhelm-ui/js-tests/`.

- Toolbar browser contracts:
  `npx playwright test tests/header.spec.ts tests/modal-layout.spec.ts tests/quick-launch.spec.ts tests/replace.spec.ts tests/restart-with.spec.ts tests/terminal-restart.spec.ts tests/session-confirmations.spec.ts tests/session-view-claims.spec.ts`;
  350 passes, 175 in each engine, including migrated callers, confirmation consent and lifecycle claims; recorder
  `21b613cb-bde7-418a-8d79-b3192c4ce660`. This tested the uncommitted toolbar above base `e72dc85a`; the later
  menu-copy, glyph and tooltip corrections preserved these operation paths.
- Corrected toolbar geometry, keyboard, consent, tooltip placement and independent Restart-with availability:
  `npx playwright test tests/header.spec.ts tests/restart-with.spec.ts tests/tooltip.spec.ts -g 'tabs and actions share|split menus keep keyboard|header actions stay|unavailable restart with stays|named restart with requires a target|hovering a row mark shows|keyboard focus shows|the version at the top edge'`;
  16 passes across both engines; recorder `18df6126-26f1-459e-b694-0ce0d204a812`. The final narrow test also checked
  actual composited explanation contrast in both themes:
  `npx playwright test tests/restart-with.spec.ts -g 'unavailable restart with stays'`; two engine cases passed,
  recorder `8b80ba3f-13f3-420b-8153-069b74ef6ca8`.
- UI unit contracts: `cargo nextest run -p farhelm-ui --lib`; 805 passes, no skips, recorder
  `53d2cea2-5c6a-4e29-aa1a-eee7d25c4b93`, at the same earlier toolbar source as the 350 browser cases; reused for the
  unchanged operation and shared-menu contracts.
- Tooltip JavaScript: `node --test tooltip.test.js`; 11 passes, recorder `738e017c-b3b0-4d3e-805e-95962a18759a`. Final
  toolbar screenshot capture:
  `FARHELM_VISUAL_REVIEW_SCOPE=session-toolbar scripts/docs-screenshots.sh --only visual-review --no-build`; five cases
  passed and 36 PNGs produced, recorder `8ca5f229-06b0-4ad2-bb49-5b1da626bd5b`. The recorder retained the named scope
  input with `--keep-farhelm-env FARHELM_VISUAL_REVIEW_SCOPE`.
- Native tunnel seams: `cargo nextest run -p farhelm-ui --features desktop -E 'test(tunnels::tests::)'`, six passes and
  982 selected-out tests, recorder `126ed7c4-440c-4696-bc76-4ee816631e5c`. This is the integrated tunnel behavior; later
  corrections affected presentation, decorative markup and test oracles. A subsequent native compile passed with the
  revised markup, but that compile's exact command identity and recorder ID are unavailable. Contrast and CSS role
  tests: `node --test app-css-contrast.test.js app-css-tokens.test.js`, 36 passes, recorder
  `07cdc704-006b-45de-ac46-6dccbe225e82`.
- Tunnel geometry and settled hover/keyboard contrast:
  `npx playwright test tests/header.spec.ts -g 'native tunnel preview'`, two engine cases passed, each exercising both
  palettes and seven states, recorder `f82ab6ae-4574-417d-a9f8-93b6bed20817`. It also checked room for a visible host
  ellipsis, selected shell text at 650, 800, 820 and 900px, and wide adjacency to the actual tabs. Matching web build
  passed: `dx build --package farhelm-ui --platform web --release` from `crates/farhelm-ui/`, recorder
  `3377467e-080e-4e1e-8a02-19894498e62f`. The final tunnel screenshot capture used
  `FARHELM_VISUAL_REVIEW_SCOPE=session-tunnels scripts/docs-screenshots.sh --only visual-review --no-build`, with the
  same explicit scope retention. It passed five cases and produced 36 PNGs, recorder
  `2404c26b-6ffd-460b-9ffb-0c02f9c6bbf5`. Build, browser and capture source fingerprints match; later changes are
  limited to commit metadata and this report.
- Toolbar UI Clippy and native product-library Clippy passed. Desktop all-targets Clippy failed on an existing,
  unchanged approval-card test's type-complexity lint, also present on the plan's base main. That warning was not
  suppressed or changed in this plan; the native tunnel tests compiled and ran successfully. The product-only lint
  result predates the final decorative markup correction; the subsequent native compile covers that correction. These
  Clippy runs' exact command identities and recorder IDs are unavailable, so they are limited observations rather than
  reproducible recorded gates.
- Source-only checks passed: `cargo fmt --all -- --check`, `git diff --check`,
  `dprint check SPEC.md SPEC_impl.md releasing/changelog.d/steel-session-toolbar.md`,
  `python3 releasing/check-changelog.py format`, and `<isolated-python> -B scripts/check-test-sleeps.py` (322 delays,
  none without a rationale). These source checks have no recorder IDs. No whole-workspace, installer, release or
  live-install checks were warranted by this UI change. The backend build from recorder
  `f1c563f1-9cd8-4c58-a607-99eec3c0dcbf`, `cargo build -p farhelm`, was reused because backend code did not change.

Development failures remain retained privately. Two tunnel-browser retries failed on assertions that required the
selected tab's close-button sibling or trailing button padding to be visible, although its text was readable; recorders
`b9eac0f5-97bb-4bf0-8fa8-c74bdc91db99` and `05964f9a-ac91-4308-93dc-bc0b178b4b44`. The corrected test measures actual
selected text and separately checks room for the whole tab. One build used a different installed dx executable and
failed before compilation, recorder `bedc0644-3338-4a76-89a8-ec8289685f62`; the retry uses the same executable as the
earlier successful build. These observations do not establish a latent flake.

## Review gate outcome

The toolbar passed its single gpt-6.1-sol/high code reviewer and single Opus 5.5/high visual reviewer. Follow-ups used
the same reviewers. The visual reviewer inspected every captured state and confirmed corrections for tooltip overlap,
unavailable cues, distinct action glyphs, menu wording and readable unavailable explanations.

The tunnel PR passed its own single gpt-6.1-sol/high code reviewer and single Opus 5.5/high visual reviewer, with
corrections returned to those same reviewers. The final visual round inspected all 36 updated captures and found no new
defect. Corrections preserved readable tabs, put the pill beside actual tabs on wide windows, kept visible host
abbreviations honest, and aligned list focus, busy controls and errors with the shared design. The tighter action
spacing described above was deliberately retained after scope reassessment. The normal full-mapping tooltip may cross
the sidebar boundary; that minor recommendation was also declined to retain the existing tooltip behavior.

### Landing

Landed with two small fixes at landing. The open session's top row now follows Steel: Restart and Replace are split
buttons whose menus also offer Restart with and Replace with, the text size buttons start the action group on the right,
and the desktop app's tunnels moved from their own row above the terminal into a pill beside the terminal tabs, whose
list closes tunnels one by one or all at once.

- #2753: Restart and Replace menus in the session's top row.
- #2754: the tunnel pill and its close controls.

#### What landed since the stack was based, and whether it interacted

Nothing but plan bookkeeping landed since the plan was based. A fresh-context reviewer checked, on purpose, the failure
that stopped the Steel sidebar's first landing: no test in a file this plan did not touch still looks for a renamed or
moved toolbar control. The full browser suite confirmed it (below).

While the stack was merging, main gained two trailing-review records (#2752, #2755), documentation only. One finding
they queued says #2740's approach can hide the link refusal's explanation under an unrelated overlay, such as an agent's
request card, while a header question is open; the link is still refused. The tunnel fix below uses the same approach,
so the same trade-off applies to the tunnel failure notice; triaging that finding covers both.

#### Fixes made at landing

- The new tunnel pill shows a tunnel failure as a floating notice that hangs from the top row, drawn above the header's
  Restart, Replace and Delete questions. If a tunnel failed while one of those questions was open, the notice could
  cover the question's description of what the answer will do, which the app's layering rules say must never be covered.
  While a question is open, the notice now drops below it and comes back when the question closes, the same approach
  #2740 took for the explanation shown when a terminal link click is refused. In the rare case that a tunnel failure is
  showing, a question is open and a link's destination preview reaches that corner, the lowered notice still counts as
  covering the preview, so that link click is refused rather than allowed, which errs on the safe side. A second
  fresh-context reviewer checked the fix.
- SPEC.md and SPEC_impl.md still said the text size buttons sit at the end of the terminal tab strip; they now say they
  start the action group on the right, as the new top-row description and the code have them.

#### Things you should know

- While tunnels exist, the tighter spacing in the top row also shrinks the Cancel and confirm buttons of the header
  questions to 2 pixels of padding. This was found by reading the stylesheet, not seen in a screenshot; a one-line fix
  (limit the tight spacing to the toolbar's own buttons) would undo it, and it was left for a follow-up because it is
  cosmetic.
- An agent's request card can hide part of the open tunnel list or the floating tunnel failure. The plan's specification
  changes dropped the earlier promise that request cards never cover the tunnel controls; no recorded decision of yours
  covers that, so it is yours to accept or reverse.
- In the tunnel list, Close all and Dismiss have no hover text.
- The tunnel pill was checked through a copy of its desktop markup in the browser and Rust tests; no one clicked the
  real pill in the desktop app, and the macOS title band was not observed.
- The docs website's pages on working in a session, approving agent requests and stopping, restarting and resuming still
  describe the old toolbar; they change with "refresh the docs".
- Small leftovers for a later cleanup: an unused hidden description of Restart with, a few stale code comments, the
  layering comment's list, which does not yet name the header menus or the tunnel list, and SPEC_impl.md's list of
  surfaces that refuse link clicks, which does not name the new tunnel list or tunnel failure notice.

#### Checks

This round landed only this plan. On the stack as delivered:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint
  `cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`,
  `cargo check -p farhelm-desktop`, the check that every test delay states its reason, `dprint check` across the
  repository, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (two warnings, both
  about citations main already had), the workspace debug build, the UI's release web build and
  `scripts/check-desktop-assets.sh`: all passed. The workspace lint does not cover the desktop app's own code, so the
  report's note that the desktop app's all-targets lint fails on an older warning still stands; it was not rerun.
- The UI's Rust tests, `cargo nextest run -p farhelm-ui`, run `05a68647-25e5-4a06-b724-5c61149a1a1f`: all 805 passed;
  its desktop-build tests, `cargo nextest run -p farhelm-ui --features desktop`, run
  `4fddafe1-8de1-45c7-8fd4-5ee3519c4529`: all 988 passed. The UI's JavaScript and stylesheet tests (`node --test` in
  `crates/farhelm-ui/js-tests`), run `11db1ff3-6a04-4d85-b7a5-3471d82c133f`: 358 passed.
- `npx playwright test` in `e2e/`, the whole browser suite on Chromium and WebKit, three workers each with its own helm
  and supervisors, no retries, run `e7ad1ed0-770d-4807-989e-19a89a462326`: 2755 passed, 14 skipped by design (WebKit
  cannot be granted clipboard access, tests that need a real signed-in Claude, and a Chromium-only pixel comparison),
  and 1 failed on Chromium, the known open flake in `e2e/tests/terminal-reconnect.spec.ts` about a browser tab opened
  unaware of a takeover (in FLAKES.md and the Deflake list), in reconnection code this plan does not touch.

After the landing fixes:

- `dprint check` across the repository; the UI's JavaScript and stylesheet tests, run
  `def570f2-3dbc-4546-be94-924bf3284548`: 358 passed; the UI's release web build.
- `npx playwright test tests/header.spec.ts tests/terminal-link-clicks.spec.ts` in `e2e/` (the toolbar, its questions
  and the tunnel pill, and the link-click tests that hold the same layering rule for the link refusal), on Chromium and
  WebKit, run `786c6981-3d0a-4265-b643-b2f6532104ba`: all 56 passed. No test drives a tunnel failure while a header
  question is open; the layering fix was checked by reading the page structure and its stylesheet.
- Reused for the landed version: the Rust tests and the whole browser suite above, run on the stack as delivered (top
  `9911a1d9`). The landing fixes change only one stylesheet rule and specification text; the rule is covered by the
  rerun above, and nothing else they touch is executed. Skipped: the rest of the Rust workspace (only the UI crate
  changed, and only the desktop app depends on it), the desktop smoke tests (no desktop-specific code changed, and they
  do not inspect the tunnel pill) and the native Mac app (not runnable here).

The report ran selected browser specs; this landing ran the whole suite. Nothing else in the executor's report became
untrue at landing.
