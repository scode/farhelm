## What this was about

The session summary already opened beside the session list, but hover help for nearby marks still covered the list. The
maintainer chose the same beside-the-list placement for every row target, including the notification bell, with a
pointer notch and a thin ring identifying the hovered target. The summary card also needed to centre on its row.

Every session-row hover now follows that design. The box centres on its target and shifts only to fit the window; the
notch stays level with the target except where its rounded corners require a small inset. Moving between targets,
dismissing help or removing a row clears the previous ring and notch.

## Things you should know

When the window has no room beside the list, help keeps the established above/below fallback without a notch or ring.
The current session uses an inverse ring so it remains visible on the selected row, including in the light theme. The
browser checks exercised both row densities, both themes and the notification bell. Screenshots from Chromium and WebKit
were inspected, including the selected-row filled status dot and the summary card.

## Open questions and possible follow-ups

None required for this plan.

## The PRs

- [#3128: move session-row tooltips beside the list](https://github.com/scode/farhelm/pull/3128/changes).

## Checks run, reused and skipped

Run now on the final behavior:

- The complete `tooltip.spec.ts` and `tooltip-coverage.spec.ts` selection through the recorder: 40 passed, 20 per engine
  on Chromium and WebKit, with zero failures or skips. Run `bc53cbf2-5764-4704-943c-a80eec2cf351`, Playwright 1.64.0,
  three workers, zero retries and repository-built pinned tmux. The report and output are complete; cleanup finished.
- `cargo check -p farhelm-ui`, `cargo check -p farhelm-ui --features desktop` and
  `cargo clippy -p farhelm-ui --all-targets -- -D warnings` passed. CLI and web builds passed to supply the browser run.
- `cargo fmt --all -- --check`, changed-Markdown `dprint check`, changelog format and diff whitespace checks passed. The
  isolated pinned-parser delay checker inspected 420 delays with no missing rationale.

Reused within this plan: the full UI JavaScript harness passed 526 tests in run `33525315-b8c4-4cab-93f1-095199034968`;
the focused tooltip geometry/lifecycle selection passed 20 in `fd29c6ce-7fa3-44e5-8cec-0969f5974fec`. Their JavaScript
implementation and assertions remained unchanged after those runs; the later changes corrected CSS and added browser
assertions, covered by the final browser run. The full-harness result covers revision `5b101ca2e8c1`; final reviewed
behavior is `27c6ac61076a`, published as `f271267b54a3`.

Broader Rust runtime suites, desktop execution, installer checks and CI were skipped: the change affects shared UI
render attributes and tooltip styling/lifecycle, covered by compilation, the JS harness and both browser engines. Native
desktop compilation and WebKit browser coverage do not claim a manual macOS app check. Main advanced only by queue claim
changes, whose complete diff was inspected; no interaction required a rebase or another test run.

## The review gate's outcome

The required fresh-context gpt-6.1-sol reviewer at high effort found two visual defects: low ring contrast on selected
light-theme rows and status shapes painting over an inset ring. Both were corrected, with selected-row contrast and
status-outline assertions added to the browser test. The same reviewer checked the correction delta and reported no
further findings. A fresh cold reader accepted the final concise PR title; the self-explanatory description is empty.

### Landing

Landed with a stylesheet fix that keeps the same look and one SPEC.md sentence. All hover text in a session row (the
summary card, the status, the agent mark, the bell and the rest) now opens beside the session list, centred on the item
it describes, with a notch pointing back at that item and a thin ring drawn around it.

- #3128: the placement, the notch and the ring, with their tests.

#### What landed since the stack was based, and whether it interacted

Since this plan's base, main landed the named-credentials work for Tensorlake hosts (#3074 to #3126), which also changed
the UI's hosts view, its icons and its stylesheet. A fresh-context review read this change against it and against the
two other plans in this round. They do not overlap: the credentials work's new host and Settings rows keep the ordinary
hover placement, and no other browser test checks where session-row hover text goes. The other two plans in this round,
a fix for links to paths a program split across two terminal rows (sent back to its executor, see below) and moving
checkout cache cleanup to the background (landed after this one), touch different code.

#### Fixes made at landing

- The ring's color went through a variable of its own, defined only on the current row, instead of the theme's named
  colors that both the light and the dark theme declare. Farhelm's stylesheet tests refuse such a variable, so two of
  them failed. The ring now uses the theme's colors directly, in separate rules for an ordinary row, the current row and
  the status dot; nothing looks different.
- SPEC.md's general rule that hover text sits above its control now points at the session-row exception this plan added.

#### Things you should know

- The report's JavaScript test run came before its last stylesheet changes, which is why it did not catch the failure
  above.
- In the full browser run before the fix, one WebKit test of the status dot's color ("an idle session already seen uses
  its row's status foreground with the plain word") once found the dot white. It passed three re-runs on each engine
  after the fix. Whether that one failure came from this plan's original ring rules is not established.
- The review noted one visual edge case it inferred but nobody saw: if a row's status dot is redrawn while its hover
  text is showing, the ring could disappear while the notch stays. A possible follow-up if it is ever seen.

#### Checks

This round's checks ran once on three plans stacked on main `844a7209`: this one, the other plan landing in this round,
and the path-link fix that was then sent back. Before the landing fixes:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the lint of the UI's browser build,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the check that every
  test delay states its reason (`python -B scripts/check-test-sleeps.py`, 420 delays, none without a reason),
  `dprint check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no errors; three
  warnings about older text), the docs website build (`bun run build` in `website/`), the workspace debug build and the
  UI's release web build: all passed.
- Every Rust test in the workspace, `cargo nextest run --workspace --exclude farhelm-desktop` (with rootless Podman and
  its offline image present, so the containerized tests ran), run `9d012e5b-959d-47a6-b478-c48fc1463eaf`: all 5734
  passed, 69 skipped by their own design. The desktop-build tests, `cargo nextest run -p farhelm-ui --features desktop`,
  run `6f70cd74-34ec-4007-aa21-eca5d055113b`: all 1089 passed.
- The UI's JavaScript tests (`node --test` in `crates/farhelm-ui/js-tests`), run `3f1b635d-ea54-47a6-af2d-115fbfe321d4`:
  527 passed, 2 failed, the two stylesheet tests the session-row hover plan's ring color broke (fixed at landing).
- The grid terminal component's browser suite on Chromium and WebKit
  (`npx playwright test --config grid-client/playwright.config.ts` in `e2e/`), run
  `327273ed-ec2f-4f31-ad50-d1f5f5666888`: 184 passed, 2 failed, one container-session path-link test on both engines,
  caused by the path-link fix (it passed without that plan, run `afd16173-4ead-44a8-8fbc-187689c5821d`), which was sent
  back to its executor.
- The whole browser suite on Chromium and WebKit, on ten Tensorlake sandboxes at the maintainer's request (cloud virtual
  machines; each ran its share of the suite's spec files through the test recorder,
  `python3 scripts/record-test-run.py --kind development ...` running
  `npx playwright test --project=<engine>-<spec> ... --retries=0`, built locally; the sandboxes run Node 24.16 rather
  than the development machine's 24.20), runs `bee8fe6d-ae5f-492b-a9cf-438692919ea1`,
  `84e886a6-e907-400e-9c9f-22f7f8503002`, `5fb6cc43-53ae-4eb1-bd6e-f9c26451aaf8`,
  `57f343a5-cd3d-44b8-9949-7b169a3102ef`, `de04602a-31b5-413e-a9f5-6ab042b7551c`,
  `600ddcc3-e6fb-49e0-938e-d2d2e0ae8bda`, `3ad00bf1-15f7-4bd0-9a5d-52dde95d1a58`,
  `355178cf-8854-4faa-b7bd-4fdbdd121568`, `911472df-570e-4ff5-a133-8a053f9a96e2`,
  `150b5c57-6706-441a-9053-43e80b235a42`: 3018 passed, 14 skipped by the specs' own design, 2 failed, both on WebKit in
  run `600ddcc3`. "A pending checkout folder save survives closing settings" is a recorded flake with the same
  signature. "An idle session already seen uses its row's status foreground with the plain word" saw a white status dot
  once; it passed three re-runs on each engine after the fixes below. Then the suite's old-terminal set (the tests that
  only apply to sessions still in old terminals) on four sandboxes with the same build, runs
  `994df3b7-e0a0-4bb7-a565-58654f5db524`, `f8cb1564-f01f-425e-93f0-5fccb47ee5cf`,
  `21865731-ecc8-4245-9caf-a82fdcc06a42`, `e0baf14f-7391-413d-80f8-f46cbd2c2734`: 107 passed, 1 skipped by its design.
- After the landing fixes, with the path-link fix taken out of the stack: the UI's JavaScript tests, run
  `6f6f66b1-643c-4e2c-955d-ba1f43442cf5`: all 526 passed (three fewer than before: the path-link fix's own tests went
  with it); `dprint check`, the changelog format check, the docs website build, the workspace debug build and the UI's
  release web build: all passed. On Tensorlake sandboxes with a new build: the tooltip spec file on Chromium and WebKit,
  runs `9dc24879-9764-4565-9786-43b5592b924d` and `8df616f4-2fd7-4833-a96f-7f553e645a82`: 17 passed on each engine; and
  three repeats of the idle-session status test on each engine, runs `8fef0602-cb27-4a2c-a3a7-82eeefe4be15` and
  `2709932a-61e9-4c63-a6dc-2d2562ebd3da`: all passed.
- Reused for the landed tree, from the runs before the fixes: the lints, the Rust and desktop-build tests, and the rest
  of both browser runs and the grid terminal suite. The fixes rewrote three stylesheet rules with the same colors and
  changed SPEC.md and a guide page; taking the path-link fix out removed only its terminal link code, its tests and its
  docs. The grid suite's only failures were that plan's, and nothing that landed touches the grid terminal.
- Not run: the doctests (no doc examples changed), the desktop smoke tests and the installer tests (no desktop shell,
  packaging or installer code changed), the docs website preview (the build above passed; no page was viewed), and the
  native Mac app (not runnable on Linux).

Nothing else in the executor's report became untrue at landing.
