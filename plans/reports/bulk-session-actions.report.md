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
