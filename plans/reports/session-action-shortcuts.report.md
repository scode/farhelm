# Session action keyboard shortcuts

## What this was about

Deleting or renaming the open session required the mouse. Cmd+W also closed the Mac app, so it could not serve as a
delete-session shortcut until the native menu stopped claiming it. Escape did not cancel the header's Restart, Replace
and Delete questions.

The stack adds Delete on Cmd+W in the Mac app and Ctrl+Shift+Backspace in browsers. Like the header button, it asks
before deleting, even for an ended session, unless “skip session confirmations” is enabled. Cancel declines it; Escape
also declines it and returns focus to the terminal. With that setting enabled, deletion starts immediately. With no
session open, the key does nothing. Rename uses Cmd+L in the Mac app and Ctrl+Shift+L in browsers, opens the existing
dialog for the open session even when its row is filtered out, and returns keyboard focus to the terminal on close.
Escape cancels all three header questions. The keys appear in tooltips and the Delete/Rename documentation.

## Things you should know

The native Mac behavior has not been exercised. Linux checks compile the desktop menu construction and test the
production menu description, including a guard against restoring the Close Window item that hard-wires Cmd+W. They do
not prove actual Cmd+W/Cmd+L delivery, Cmd+Q, or window-close persistence on macOS. Browser WebKit is renderer-family
coverage, not a native Mac test.

Review caught a stale-title case: another client could rename the open session while this client's original selection
still held the old name. Shortcut Rename now follows accepted title refreshes independently of sidebar filtering, so an
untouched Save preserves the refreshed raw title. It also preserves the existing protection against delayed work taking
focus from an unrelated control.

The broader browser command failed: 256 cases passed and the populated session launcher's 1440-by-900 no-scroll
assertion failed once in each engine. An exact two-engine reproduction failed the same way. The launcher controls, its
layout rules and that test are unchanged by this plan; neither failing scenario presses a session shortcut. The failures
and their screenshots/traces are retained as a separate layout follow-up. This is not a green broad run, and the failure
has not been classified as a flake.

## Open questions and possible follow-ups

Use the devbuild flow for a native Mac pass of both shortcuts, no-session Cmd+W, the menu, Cmd+Q and the window close
button, including saved size and position. Reconcile the launcher's populated-window sizing expectation with its current
conversation, initial-prompt and workspace-trust controls in a separate layout follow-up.

## The PRs

- [#2218: Escape cancels header confirmations](https://github.com/scode/farhelm/pull/2218/changes).
- [#2221: Delete the open session with a shortcut](https://github.com/scode/farhelm/pull/2221/changes).
- [#2230: Rename the open session with a shortcut](https://github.com/scode/farhelm/pull/2230/changes).

## Checks run, reused and skipped

Runtime commands use the test recorder. Browser selections use Chromium and WebKit, one worker and zero retries; nextest
uses the pinned runner, four slots and zero retries. Successful runs cited below have complete output and, where
applicable, complete runner reports. Selected-out Rust tests are distinct from runtime substrate skips.

- Ran `cargo clippy --all-targets -- -D warnings` and `cargo check -p farhelm-ui --features desktop` after the title
  repair; both passed. Ran the full UI JavaScript harness with `node --test`: 273 passed, zero skipped, recorder
  `043a8f19-47f4-49e5-9907-c24f4f853eeb`.
- Reuse the full UI Rust unit run on `042be96da56a` plus its recorded Delete working diff: 514 passed, zero skipped,
  recorder `bd88adb9-fea2-472d-bcd5-ccf8683f9e7e`. This covers the portable production-menu guard and unchanged pure
  helpers. The later focused list/rename helper run on `5223acef34c3` plus its recorded Rename working diff passed 11
  selected tests with 503 selected out, recorder `82334bf3-0d2a-407d-bc19-0e716d62c620`; the title bridge does not alter
  those helpers. Final Clippy compiled their current test targets.
- Early focused browser checks passed: header Escape 2/2, recorder `b676217f-53c8-404d-b51e-2c730004e9dd`; Delete 6/6,
  recorder `a43e974f-288e-4606-93ff-365095d9de63`; initial Rename 2/2, recorder `2553bb07-351d-49b7-a734-28a12544c673`.
  These establish their respective slices before the final title repair; the strengthened external-rename test then
  passed 2/2 on the final rebased tree, recorder `5a0af759-4eef-4b4d-8d2d-e9d3858c5ce3`, including raw-title
  preservation and reopening after local Save. The final interaction selection also passed 26/26, recorder
  `a24f8f9a-c94d-4948-97bd-ecd78abe29ed`. It covers Delete, all header Escape questions, both quick-switcher chords,
  sidebar selection and filtering, stable row Rename, the two held-save focus guards and tooltip coverage. Combined with
  the exact Rename run, this checks all four changed session-shortcut cases on the final bundle without repeating the
  already-proved Rename case.
- The required broader selection covered the changed shortcut and confirmation specs, quick switcher, sidebar and
  tooltip coverage: 256 passed and 2 failed, recorder `c57859b7-4277-40a5-ba63-9f287aa093fa`. Both failures were the
  same launcher layout assertion. Exact reproduction `4f84e005-d235-4494-80cf-e2bdb4ae7ba2` failed that one case in both
  engines. The first reproduction attempt, `707d552f-28da-4de6-b430-26114f191fa2`, selected no tests because of an
  anchored grep; it supplies no case evidence. These commands are retained failures, not passing checks. They ran before
  the final title repair and rebase; the clean final focused runs above supply the updated interaction evidence.
- Rebased onto `54f43703` after the keyboard-scroll plan landed. Read all upstream changes and compared each review
  slice against its prior revision: no patch changed. The new named Tab stops concern text-only host, SSH, feedback and
  approval areas; the shared text block's optional label defaults absent, and Rename uses a different text component.
  Existing native and pure-unit evidence remains applicable. The final bundle and focused browser checks cover the
  combined tree's navigation, dialogs and tooltips.
- The native harness executable came from the earlier successful `cargo build`; its exact build revision was not
  retained, so that build supplies no revision-specific evidence claim. No backend code changed, and the final browser
  runs exercised that executable as their owned fixture. Final `cargo fmt --all -- --check`, configured changed-file
  `dprint check`, changelog-format, the sleep checker (302 delays, zero missing rationales), and whitespace checks
  passed before the clean rebase. The title-repair and final combined web bundles built successfully. Changelog-format
  passed again after the rebase, covering 15 fragments; whitespace stayed clean. Main advanced afterwards only for
  queue/report bookkeeping, so no further rebase or runtime repetition was needed.
- Skipped the full workspace/browser batteries and CI dispatch: the plan requests selected integration coverage and
  there is no uncovered backend change requiring a full battery. Skipped website build and preview: the docs changes are
  incidental prose with no heading, link, asset or generation changes. Native Mac runtime checks remain manual.

## The review gate's outcome

Each slice received the required single fresh-context gpt-6.1-sol high review. Escape and Delete had no findings.
Rename's stale-title finding was fixed; its source recheck and the added external-rename regression review found no
remaining issues. Each commit/PR message passed its cold reader.

### Landing

Landed on its own in round 84, as #2218, #2221 and #2230, merged bottom-up. The PRs are unchanged from what the report
describes, apart from a merge with the quick-launch key sequences that landed just before (below).

#### What landed since the stack was based, and whether it interacted

Since this stack's base, main gained the quick-launch template sequences (#2220, #2225, #2228), which add their own
window-wide keyboard listener, edit the session list view, and amend the same keyboard sections of SPEC.md and
SPEC_impl.md. The landing monitor read the two features against each other, and a fresh-context reviewer did the same
independently. Neither found a functional conflict:

- The keys do not collide. Delete is Cmd+W in the Mac app and Ctrl+Shift+Backspace elsewhere; Rename is Cmd+L in the Mac
  app and Ctrl+Shift+L elsewhere. A launch sequence starts with Cmd+X in the Mac app or Ctrl+Shift+X elsewhere, then
  takes number keys (bare or with the number-shortcut chord), the ⚡ chord, Enter and Escape.
- No key is consumed by both features. Pressing Delete or Rename while a launch sequence is waiting cancels the sequence
  and then acts, as the quick-launch rule "any other key cancels and reaches its normal target" says. Delete still asks
  its usual confirmation unless session confirmations are skipped. In the reverse order, with a Delete question showing,
  a sequence started afterwards owns Enter and Escape, so its Enter cannot confirm the question.
- The input-method guard stays first among the window listeners, and these shortcuts ignore keys during text
  composition. The menu focus, keyboard scroll region and template name changes already in the base do not interact.
- The SPEC.md and SPEC_impl.md amendments from both plans merge cleanly and agree.

#### Fixes made during the landing

The Rename PR (#2230) conflicted with the quick-launch landing in two places, both because each side added code at the
same spot: the interface's top-level component, where each adds its own state, and the session list view, where each
adds hidden trigger buttons. The landing kept both sides in each place. The window keyboard listeners run in the order
they are installed, so a listener that consumes a key hides it from later ones; after the merge the order was already
correct: input-method guard, launch sequence, quick switcher, quick launch, template numbers, session actions. No
behavior was changed by hand.

#### Things to keep in mind

- The Mac app has not been exercised: Cmd+W, Cmd+L, Cmd+W with no session open, the rebuilt menu without Close Window,
  Cmd+Q, and the window close button with saved size and position. The report already asks for a native pass, for
  example through the devbuild flow.
- Cmd+X from the terminal in the Mac app is an existing open question, not one this stack created: the quick-launch
  landing already lists checking whether the Edit menu's Cut takes Cmd+X before Farhelm sees it. #2221 rebuilt the Mac
  menu as a copy of the previous default, Cut included, so nothing changed there; but its SPEC_impl.md text now records
  that a built-in menu item's fixed shortcut (Close Window's Cmd+W) wins before the page, which makes the concern more
  likely. The Mac pass should try Cmd+X from the terminal together with the new shortcuts. If Cut wins, the Mac app's
  menu definition that #2221 added is where the fix goes.
- In the Mac app, W sits just below 2 and 3, so reaching for Cmd+2 or Cmd+3 (a template number key, alone or inside a
  launch sequence) and hitting Cmd+W deletes the open session, immediately if session confirmations are skipped. One
  possible follow-up is for an active sequence to swallow Cmd+W; that would be an exception to SPEC.md's rule that any
  other key cancels the sequence and reaches its normal target, so it needs a decision.
- The report records a retained, unclassified launcher layout failure in a test this plan does not touch. After this
  landing the monitor ran that test on main: "composer fits a 1440 by 900 window without scrolling once a harness and
  model are chosen" fails in both Chromium and WebKit on the asserted "must fit inside its own viewport", run
  `7d8604f9-5825-4833-9529-fc9779848919`. When it started failing has not been established.
- Minor: the docs pages this stack edits write keys in bold (**Cmd+L**), while the rest of the docs write them plain.

#### Checks

Because the landing merged this stack with another keyboard feature, both features' browser tests ran together on the
merged code, and the code the merge touched was checked again.

- Run now, at the rebased tip after fresh `cargo build` and web release builds: the session-shortcut, session
  confirmation and quick-launch browser specs in full, on Chromium and WebKit, through the test recorder with the pinned
  tmux, one worker and no retries:
  `npx playwright test session-action-shortcuts.spec.ts session-confirmations.spec.ts quick-launch.spec.ts`. 76 of 76
  passed with no skips, run `a3221a40-35d6-4bb2-9944-1ccdd2002db7`.
- Run now: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo check -p farhelm-ui --features desktop`, and the UI JavaScript harness (`node --test`, 290 passed, run directly
  rather than through the recorder), all clean.
- Run now, on main after the merge (the same code): the UI crate's unit tests through the recorder,
  `cargo nextest run -p farhelm-ui --lib`, 515 of 515 passed, run `03bae717-87c1-4438-9c87-a835ae6fefcc`; `dprint check`
  on SPEC.md, SPEC_impl.md, TODO.md and the three edited docs pages; and `python3 releasing/check-changelog.py format`,
  all clean.
- Reused: the executor's test-sleep check and its focused browser runs listed in the report, for the parts of each PR
  the merge did not touch.
- Skipped: everything the report skipped, for the reasons it gives.

Nothing in the executor's report became untrue at landing.
