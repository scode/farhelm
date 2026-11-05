## What this was about

Stopping, deleting, pinning or marking several sessions read used to require repeating the action on each session. You
can now Shift-click a range from the open session, or Cmd/Ctrl-click individual rows, to mark them without changing the
session you have open. A bar offers Stop, Delete, Pin/Unpin and Mark read/unread for the marked sessions currently shown
by the list's filters.

## Things you should know

The open session keeps its own highlight. Marks have a separate side marker and tint, belong to the current page and are
not saved. A plain row click opens that session and clears marks. Escape clears them from the list; the terminal, menus
and dialogs keep their own Escape behavior. Keyboard shortcuts still act on the open session alone.

Bulk Delete asks once when any included session needs confirmation, unless Skip session confirmations is on. The
question lists each session's status, name, full host and consequence. If a session changes, its question is updated;
one that has disappeared can be removed. Confirming cannot gain an unlisted session or permission to stop something the
question did not cover. Managed checkouts keep the ordinary rule: deleting their last session moves the directory to
trash rather than deleting its files.

Stop and Delete run one session at a time on each host, with different hosts progressing independently. Queued rows
remain locked until their work settles, so deleting the open session cannot automatically open another session still
being deleted. Each failed or refused action reports on its own row, successful siblings stay successful, and marks
clear when the run finishes. A failed row can be marked again for another attempt.

The guides “Read the session list” and “Stop, restart, and resume” now describe these actions. The requested TODO entry
is removed. No new backend endpoint, bulk action with a follow-up, saved marking or shortcut target was added.

## Open questions and possible follow-ups

No maintainer decision remains. Native macOS interaction was not exercised: the requested desktop evidence is the WebKit
browser run plus desktop-feature compilation.

## The PRs

- [#3052 — Mark sessions for bulk actions](https://github.com/scode/farhelm/pull/3052/changes): gestures, distinct
  marks, the bar and bulk metadata actions.
- [#3053 — Stop or delete marked sessions](https://github.com/scode/farhelm/pull/3053/changes): paced Stop/Delete, the
  combined question, completed documentation and TODO removal.

Both PRs are drafts in one linear stack; this executor has not marked them ready or merged them.

## Checks run, reused and skipped

The final product and test tree is PR2 commit `e634a294ee2f33b15fece17691a40382b346de15`, on PR1 commit
`5a26bc3a31a1ae64fe65414f2f528f3cd311552d`. Checks ran before committing. The final browser and CSS checks captured that
working tree with fingerprint `ef23ddb3a96d83505af49c55f9206f35d801f4310c60a1d568f1a0b8f3b1ab6f`.

Current second-unit checks:

- The focused UI native selection below passed 53 tests in recorder run `62069aed-7482-49a4-8f4e-4ea67bd66745`. Held
  requests prove that queued sessions stay locked, requests run in order on each host, and other hosts progress
  independently. Confirmation tests reject old answers and keep each request limited to what its question described.
- `cargo clippy -p farhelm-ui --all-targets --features desktop -- -D warnings` passed on the final source, including the
  hover help and status-layout correction.
- `dx build --package farhelm-ui --platform web --release` passed on the final source.
- Recorder-backed Chromium/WebKit coverage of the bulk spec and adjacent session confirmations, Delete preconditions and
  shortcuts covers 52 cases. Run `6d52ac3d-8b46-4090-8a93-f943f3ba69e7` passed 46 and failed six new fixture assertions.
  Those failures remain retained: an unnecessary popup-visibility assertion, a border-box width compared with content
  width, and a completion-focus assertion racing the next automatic terminal selection. Corrected targeted run
  `1d1a5395-6818-4a6a-ac56-f013e04adb0a` passed all six affected cases. The other 46 cases are reused because subsequent
  changes leave their behavior unchanged.
- The full bulk spec on terminal-host sessions passed 28 cases, 14 per engine, in
  `d50d463b-ab05-4079-be33-acf8c3eeda1e`. Visual inspection then found the Unknown label squeezed by a long title; its
  local flex-layout fix and measured clipping assertion passed the structured-question case on both engines in
  `65b827a2-bfb2-43cb-9185-d8b0d93ac096`, and on terminal hosts in `b7eb4d2c-add1-494c-948d-c033cad51542`. Other cases
  are reused: that final change affects only the question's identity-line layout. Actual WebKit captures were inspected
  in both palettes and at narrow widths; the status, full host, consequences and answers remain readable.
- `node --test app-css-tokens.test.js app-css-contrast.test.js app-css-animations.test.js` passed 53 source checks in
  `eb86cd7e-d941-454c-b347-2ffedf729241`, covering the changed stylesheet's tokens, contrast and animation rules.
- `bun run build`, from `website`, passed with 36 pages and valid internal links. `cargo fmt --all -- --check`,
  `dprint check SPEC.md SPEC_impl.md TODO.md`, `python3 releasing/check-changelog.py format`,
  `python -B scripts/check-test-sleeps.py` with the isolated interpreter, and `git diff --check` passed. The delay
  checker inspected 412 delays with none lacking a reason. `python3 scripts/check-agents-md.py` reported zero errors and
  three pre-existing warnings about older instruction citations.

Reused first-unit evidence includes 36 native marking/movement tests (`8b6971fe-25ae-4df2-9286-6e7cfd35f1dc`), nine
toolbar/Trash ownership tests (`206949af-952e-4d4e-bab8-5c551c027176`), and 24 adjacent Pin/sidebar browser cases
(`8a9b7f44-f02a-4779-a8e1-b7b17c79b8ce`). These cover the reviewed marking and ordinary-control behavior retained by the
second unit. These selections' covered code is preserved by PR1 commit `5a26bc3a`; its other subsequent edits were
reviewed separately. The second unit also reruns the whole bulk spec on terminal hosts. The 53-test native selection's
production logic is unchanged after its run; later edits add hover text and adjust modal CSS.

These were the portable child commands for the recorded selections above, in the same order as the corresponding native
or browser results. Runtime commands used the recorder, pinned nextest with four slots, zero retries, and up to three
browser workers as applicable. The host-terminal runs retained `FARHELM_E2E_SUITE_TERMINALS=host` through the recorder;
the ordinary runs left it unset.

Current 53-test native selection:

```sh
cargo nextest run -p farhelm-ui --lib -E 'test(/^list::lifecycle::tests::/) | test(/^list::marking::tests::/) | test(/^list::shared::tests::/) | test(/^list::view::tests::automatic_selection/) | test(the_row_operation_exclusion_still_blocks_a_second_operation) | test(/^ops::tests::/) | test(the_delete_guard_matches_what_the_prompt_said)'
```

Browser selections: initial 52 cases, corrected six cases, full 28-case host-terminal run, and the final two-case layout
selection run once ordinarily and once on terminal hosts:

```sh
npx playwright test 'bulk-session-actions.spec.ts|session-confirmations.spec.ts|delete-precondition.spec.ts|session-action-shortcuts.spec.ts'
npx playwright test 'bulk-session-actions.spec.ts' -g 'bulk Stop reports|bulk Delete rewords|bulk Delete keeps'
npx playwright test 'bulk-session-actions.spec.ts'
npx playwright test 'bulk-session-actions.spec.ts' -g 'bulk Delete rewords'
```

Reused first-unit native 36-test and nine-test selections, then the 24-case Pin/sidebar browser selection:

```sh
cargo nextest run -p farhelm-ui --lib -E 'test(list::marking::) | test(marked_rows_keep_open_session_hooks_and_accessibility_independent) | test(repeated_parent_refreshes_do_not_rerender_an_unchanged_row) | test(anchored_popovers::tests::)'
cargo nextest run -p farhelm-ui --lib -E 'test(list::trash::tests::) | test(toolbar_renders_the_projected_count)'
npx playwright test 'pins\.spec\.ts|sidebar\.spec\.ts' -g 'pin from|menu moves|new pins remain|drag pins|pin dragging|an unanswered view operation|row actions exist|opening another row.s menu does not change|launch composer autofocuses|manual mark-unread on the open session|dot click marks a different row|listing refresh with nothing layout-relevant'
```

Failed development observations remain retained:

- The current native selection above failed its dialog-content test in `5da1a816-c06b-4b5d-8620-d16ec6de2671` (then 52
  tests: 51 passed). Its observer omitted static template text. The observer was corrected without dropping the content
  assertions; the full selection subsequently passed, including the parent's 53-test run above.
- The first-unit range/text-drag selection failed in `bcdc0b88-c41d-4bfb-97ff-e9ecdbb942f6`. A WebKit-only diagnostic
  `d94fe403-c45d-46a9-a170-3f2f5272d22f` established that ordinary native buttons also did not offer the assumed
  draggable text. The test now checks actual page text instead. These used
  `npx playwright test 'bulk-session-actions\.spec\.ts' -g 'range and toggle marking'`; the diagnostic selected
  `--project=webkit-bulk-session-actions --retries=0` through generic recording.
- The full bulk spec failed premature initial-focus waits in `e4723b45-236b-4cac-b035-9f142d632b95`. Mouse cases now
  wait for the requested terminal to be revealed; terminal-key cases establish focus explicitly. A host run of
  `npx playwright test 'bulk-session-actions\.spec\.ts' -g 'marking retains New|host attention movement'` failed a
  fixture's wrong helper argument in `215597a9-1b98-4703-93ca-b7a99a7c3deb`, which was corrected.
- The full host-terminal bulk spec in `689661be-33cc-4d51-ac42-604f0422d9cd` passed 13 cases and encountered an
  agent-start failure in the metadata fixture. Its exact cause was not established. That ended-state fixture was made
  independent of Stop, and passed afterward on both engines and terminal kinds. Both full-spec attempts used
  `npx playwright test 'bulk-session-actions\.spec\.ts'`.
- Native attempt `de3d4511-f7f8-4ed7-9789-026d629817b5` executed no tests: its UI filter had a malformed
  `test(/^list::view::tests::identity_pending_)` expression. The filter was corrected; this is not a runtime failure or
  a passing observation.

The complete current bulk runs cover the corrected first-unit cases. These are development observations, not claims of
latent flakes or passing failed runs.

The full workspace, installer, provisioning, release and native Mac suites were skipped: this change affects the
session-list client, and the focused native, browser, web and desktop compilation checks cover its concrete risks. CI
was not dispatched; this repository's CI is manual-only.

## Review gate's outcome

Each PR received the required fresh-context gpt-6.1-sol/high adversarial review. Findings were addressed, including
keeping an open Trash dialog and ongoing cleanup usable when marked rows appear or disappear, preserving the existing
New-session shortcut action while the bar replaces its button, and adding hover help to the new lifecycle controls. The
second unit's final review found no actionable issues. An independent scope check supported keeping that Trash behavior
through the existing dialog. The final visual status correction received a bounded Sol/high review with no findings, and
a fresh scope reassessment supported the integrated design without new machinery. Commit/PR wording passed the required
cold read. No review finding was left unresolved.

### Landing

Landed with no fixes at landing. You can now mark several sessions in the list (Shift-click a range from the open
session, or Cmd/Ctrl-click single rows) and Stop, Delete, Pin or Unpin, or Mark them read or unread together, without
changing the session you have open.

- #3052: marking sessions, the marked-sessions bar and its Pin, Unpin, Mark read and Mark unread actions, with the
  browser spec and the session-list guide.
- #3053: bulk Stop and Delete, with their browser cases and the Stop, restart and resume guide.

#### What landed since the stack was based, and whether it interacted

Since the plan's base, main gained tensorlake-ui-fixes (#3048-#3051: Tensorlake suspend and resume progress, the suspend
confirmation, the recipe editor and the Tensorlake Add host dialog); nothing else landed in this round. A fresh-context
review found no overlap: the new code uses the host list only to find the local host, and none of the dialog styles the
Tensorlake fixes changed. The same review checked it against the plans that landed shortly before its base:

- Delete's consent: bulk Delete sends each session the same delete precondition a single Delete sends, taken from what
  the question showed, and a refusal appears on that session's row as it does for a single Delete. Bulk Stop asks
  nothing, as a single Stop does, so neither ends live tabs beyond what the single action would.
- Pin order: a drag starts only from the grip, the drag's own Escape never clears the marks, and a Shift-click range
  follows the same row order the drag uses while the list holds its order steady under the pointer.
- The host attention list and Hosts: the marked-sessions menu closes when a host warning appears above the list and
  moves the bar; while Hosts is open no marks count and every bulk action refuses.
- Trash: while sessions are marked, the Trash button is only hidden, so an open Trash dialog keeps its work.

#### Things you should know

- The plan's new browser test "bulk Stop paces nine rows on one host while another host progresses" failed once in the
  full suite on Chromium: on one host, two sessions were stopped in a different order from the one the test had read off
  the list. It then passed ten re-runs. The test reads the list's order before it clicks Stop, so if the list re-sorts
  in between, the test's expectation is stale while the product follows the list as shown; the landing did not establish
  whether that or a real ordering bug caused it. Because it is the plan's own new test with one failure and ten passes,
  it is not recorded as a flake, and nothing tracks it if it recurs; a recurrence would be worth recording and
  investigating, since the test already reads the order just before the click and a re-sort during the run would fit the
  failure as well.
- The browser test of hover text failed every time in this round, as on main in earlier rounds, because the plain Add
  host form's "SSH" and "Tensorlake sandbox" choices have no hover text (from #2973). Nothing in TODO.md or the review
  queue records it yet.
- If the session list fails to load while the bulk Delete question is open, confirming does nothing and says nothing.
  SPEC.md allows it; keeping the question open until the list loads again would be a small follow-up.
- Each status change on a live session re-issues the Delete question, so a Confirm click that lands at that moment is
  refused and needs a second click.
- Closing Templates or Trash while a bulk action runs tries to put focus on the disabled ⋯ button, so focus is lost.
- The review suspects, without verifying it, that Shift+Enter or Cmd+Space on a focused row may mark it rather than open
  it, if the browser passes the held key on to the click.
- Smaller: an ended session with no tabs reads "Delete anyway:" with nothing after the colon (the existing wording the
  plan asked for); the marked-sessions menu items repeat their visible text as hover text, which SPEC_impl.md says is
  unnecessary; and SPEC.md's list of confirmations that start with focus on their confirm button does not name bulk
  Delete.

#### Checks

This round landed only this plan, stacked on main `627dc733`. On the stack:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the check that every
  test delay states its reason (`python -B scripts/check-test-sleeps.py`, 412 delays, none without a reason),
  `dprint check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no errors; three
  warnings about older text), the docs website build (`bun run build` in `website/`), the workspace debug build and the
  UI's release web build: all passed.
- The UI's JavaScript tests (`node --test` in `crates/farhelm-ui/js-tests`), run `a84727a0-9a5e-42d0-9894-89029198c9ba`:
  all 527 passed. The UI's Rust tests, `cargo nextest run -p farhelm-ui`, run `d33c3fa1-1323-4816-8fbd-c8a2d48537ed`:
  all 895 passed; its desktop-build tests, `cargo nextest run -p farhelm-ui --features desktop`, run
  `0a3a26d7-6b03-4f3c-8471-30e53afc4fa1`: all 1084 passed. The whole Rust workspace with the pinned tmux,
  `cargo nextest run --workspace --exclude farhelm-desktop`, run `f73eb4f2-d5df-41b3-9926-916d708c615d`: all 5653
  passed, 70 skipped (ignored by design).
- The whole browser suite on Chromium and WebKit, on ten Tensorlake sandboxes at the maintainer's request (cloud virtual
  machines; each booted the usual three test stacks and ran its share of the suite's projects through the recorder's
  generic mode, `python3 scripts/record-test-run.py --kind development ...` running
  `npx playwright test --project=<engine>-<spec> ... --retries=0`, with the binaries and web bundle built locally from
  this stack), runs `4a668f9e-baca-4b20-be38-19482ccd2288`, `82a8ff34-43be-4fee-b4bf-ce22fdd27319`,
  `6149e681-af26-4b48-a9dd-4db02eb8c3b7`, `ab912df5-65aa-4e5a-a1b6-433fba1581c8`,
  `69205a4b-0b1e-4713-ade3-e78b51012f9b`, `6de6e6be-dbe8-441b-8e18-f2da02830bad`,
  `9afdafba-923d-4dc5-ac15-1b1cd2635155`, `6022701f-d571-4419-a78b-b1547487552f`,
  `bdc062f4-b027-46c8-8dde-0ab1a3c88ca2`, `1b760148-02b4-43df-beae-203e056ab313`: 3075 passed, 18 skipped by the specs'
  own design, 3 failed. Then the same suite with every stack's terminals in terminal hosts, since the plan's new spec
  joins that run's list (`FARHELM_E2E_SUITE_TERMINALS=host`; this run covers only the specs listed in
  `e2e/host-terminal-specs.ts`, so it holds fewer tests), runs `25f1728f-9e0d-43f0-85b9-55239e150f80`,
  `e6f07923-a365-47cf-b111-afd0d7d53cb2`, `77ddde1d-eac9-48c2-81af-81cb046c7502`,
  `cf209d3c-e234-4e9b-bdef-3317505218a0`, `70620ae9-eae6-4220-af10-5430cc2d9679`,
  `55bc7668-e195-4157-bb3f-6265da3c1307`, `06d8be44-2eae-41f5-afad-d36c92534a81`,
  `63394f54-9b33-4fbb-841f-874d84e18ec6`, `0ce6af40-d9f5-488c-b849-afb2b9252517`,
  `532f9738-391d-4cb1-82ed-c0e3d3adc98c`: 2110 passed, 75 skipped by the specs' own design, 3 failed. Of the six
  failures:
  - "Sidebar surfaces give every control hover text" failed four times, in both engines in both suites (runs `9afdafba`,
    twice, `25f1728f`, `63394f54`), on the Add host form's "SSH" and "Tensorlake sandbox" choices (see Things you should
    know).
  - The launcher's "Runs in YOLO mode" step on WebKit, in "create-intent-key-rebinds-on-host-change: a new target is a
    new intent" (run `cf209d3c`), a recorded flake.
  - The plan's own new "bulk Stop paces nine rows on one host while another host progresses" (Chromium, run `6de6e6be`;
    see Things you should know), which then passed five times on each engine, also on sandboxes, Chromium run
    `8aaac66b-5db7-45f2-a0dc-6b6b55fc5fc6` and WebKit run `2ffb7b31-cbec-411c-9233-fbb124cfb39a`.
- Not run: the doctests (no doc examples changed), the separate host-terminal browser configuration, the desktop crate
  and asset checks and the CentOS provisioning test (none of their code changed), the grid component's browser suite (no
  grid code changed), the desktop smoke tests and the installer tests (no desktop shell, packaging or installer code
  changed), the development screenshot review (no shot changed), the docs website preview (the build above passed; no
  page was viewed) and the native Mac app (not runnable on Linux).

The executor's report calls the PRs drafts and skips the whole Rust workspace run, which the landing ran; what landed is
its two commits rebased onto current main, identical in content to the ones it tested. Native macOS interaction was not
exercised by the landing either. Nothing else in it became untrue at landing.
