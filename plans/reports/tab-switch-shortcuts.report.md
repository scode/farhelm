## What this was about

With several terminal tabs open, you previously had to use the tab strip to move between them. On macOS, `Cmd+Shift+[`
now moves to the previous tab and `Cmd+Shift+]` moves to the next. Linux uses the corresponding `Ctrl+Shift` chords, as
agreed in the plan. The shortcuts wrap at either end, repeat while held, and focus the chosen terminal. The strip
scrolls to show its tab.

## Things you should know

- Dialogs and menus keep keyboard focus. The shortcuts also leave an active input-method composition alone and take no
  action when the main terminal is the only available tab.
- The source review found that a session can list more shell tabs than the app can display as terminals. The shortcut
  skips any tab without a terminal it can focus, as well as a tab whose close has already been confirmed.
- The Keyboard shortcuts dialog and the docs pages for keyboard shortcuts and working in a session describe the new
  keys.

## Open questions and possible follow-ups

None raised by this plan.

## PRs

- Add keyboard shortcuts to switch terminal tabs. [#4983](https://github.com/scode/farhelm/pull/4983/changes)

## Checks run, reused and skipped

- All 90 selected GUI tests passed on `914895ed`, run `d25d981f-6b10-47c9-87d3-61eec0831acb`. The selection covered
  physical key matching and held repeat, both platform shortcut tables, wraparound, focus and scrolling, confirmed
  closes, the tab attachment limit, and dialog and input-method guards.
  `cargo nextest run --locked -p farhelm-gui --lib -E 'test(/^actions_tests::/) | test(/^shortcuts::tests::/) | test(/^terminal::set::tests::/)'`
- The docs passed `bun install --frozen-lockfile` and `bun run build` on `5b1e2e1a`. The website inputs are identical on
  `914895ed`. Both pages rendered and all internal links passed.
- The selected light and dark `keyboard-shortcuts-dialog` screenshots matched their references on `914895ed` in run
  `7302ad67-14e8-4ad0-8b8d-2e338e064ffd`. The starting viewport ends above the new rows, which are in the scrollable
  body. The platform table tests verify the new row names and chords. The recorder ran
  `python3 scripts/check-gui-goldens.py --scene keyboard-shortcuts-dialog --jobs 2 -j 4`.
- Focused Clippy passed on `914895ed` with `cargo clippy --locked -p farhelm-gui --lib --tests -- -D warnings`.
  `cargo fmt --all -- --check`, `dprint check` on the changed docs, specs, TODO and fragment,
  `python3 releasing/check-changelog.py format`, `git diff --check` and `python -B scripts/check-test-sleeps.py` also
  passed.
- Main through `b625e36b` changed the cache sweep and Tensorlake setup sections of the implementation spec. The
  tab-switch behavior and the shortcut table were unaffected, so the recorded checks still apply.
- This run used the local Linux GUI harness. It covered simulated macOS key identities and the macOS shortcut table.
  Testing keys in a native macOS window would require a Mac.

## Review gate

Independent gpt-6.2-sol source review at high effort approved the corrected revision. Its findings led to the
attachment-limit guard and regression, and to the expected rows in both platform tables. The screenshots confirmed that
the visible part of the dialog still matches in both appearances. The new rows are below it and covered by the platform
table tests.

### Landing

#### What landed with it

Only this plan landed this round, through its PR and the landing fix
[#4989](https://github.com/scode/farhelm/pull/4989/changes). Since the plan's PR was written, main gained only the
Tensorlake setup and repository cache fixes and a review queue triage record, none of which touches the desktop app's
shortcuts or terminal tabs, so nothing could interact with it.

The landing's review read the change against SPEC.md's keyboard and focus rules and found the shortcut itself right:
previous and next pick the right tab and wrap at both ends, skip tabs whose close was confirmed and tabs beyond the 32
the app can show as terminals, scroll the tab strip to the chosen tab, and give it focus the usual way. A held chord
repeats, the key never reaches a terminal program or a text field, the chords match the physical bracket keys whatever
the keyboard layout, on macOS and Linux, and nothing else uses these chords. Every case SPEC.md says the shortcut does
nothing in has a matching guard in the code; three of those guards had no test that could fail, which the fix below
addresses. The review counted no blockers, one should-fix (fixed below) and five small points; one is fixed below and
the rest are in the last section.

#### Fixes made while landing

- **The tests of three guards could not fail if the guard were removed (the review's should-fix).** The tests for an
  open dialog, input-method composition in the shown terminal, and a session with no usable terminal (the card covering
  it) each pressed previous and then next in a session with one shell tab. That pair comes back to the main terminal
  either way, so if someone later dropped a guard, the shortcut would switch the terminal behind an open dialog and the
  tests would stay green. #4989 makes each test press one chord at a time while the guard applies and check after each
  press that the shown tab did not change, and has the no-usable-terminal test show that one press does switch tabs once
  the terminal is usable again; with each of the three guards removed in turn, the test of its case now fails, as the
  Checks show. The report's claim that these guards are tested holds only from this fix on.
- **A code comment credited the shortcut's modifiers to the old app.** The old app had no tab switching shortcut; the
  modifiers come from the plan. #4989 corrects the comment.

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. All the checks below ran
on the plan's PR rebased onto main as it stood at the start of the landing (`3c158f6e`); the rebase conflicted only in
TODO.md, where both sides removed entries and I kept both, and every other file's change is exactly what the executor
pushed. The rebased commit was `154af560`. The only later change is the landing fix
[#4989](https://github.com/scode/farhelm/pull/4989/changes), which changes two tests (covering three guarded states) and
one code comment; after it only the following ran again: the two changed tests on the test worker (recorder run
`82f366b8-3972-45c8-a7b5-6ca4101b71ff`, both passed); the same tests with each of the shortcut's three guards removed in
turn, to prove the tests now catch its absence (without the open-dialog guard, recorder run
`41cd5b6a-f992-49b9-a846-b5efe86fbdda`; without the input-method composition guard,
`40e41a6d-1045-4a24-ac12-294b32ea6c0b`; without the no-usable-terminal guard, `b4d24a36-9f1a-44ad-935c-c8c0202a6f20`;
the test of that state failed each time, as it should); `cargo clippy -p farhelm-gui --all-targets -- -D warnings` on
the test worker; and `cargo fmt --all -- --check` and `python -B scripts/check-test-sleeps.py` locally, all of which
passed. The runs listed below ran before the fix, on `154af560`. That final tree is what main now holds outside the
plans queue.

- The desktop GUI crate's tests, `cargo nextest run -p farhelm-gui`, on the test worker (recorder run
  `a4864707-38c8-45f3-af31-b3f4e08c67fd`): all 3431 passed. Only that crate's code changed, so this was the narrowest
  run covering every changed file; the full workspace suite would add nothing for it.
- The desktop app's tests, `cargo nextest run -p farhelm-desktop`, on the test worker (recorder run
  `b0a46ff4-d903-4954-8b7e-97c9f600bb32`): all 184 passed.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets --features agent -- -D warnings`, on the test worker: passed.
- The desktop app's end-to-end smoke, `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run
  `a0f32c3e-5968-4939-9aca-98a61f9d8d4e`): 50 steps passed, one skipped by design on Linux (paste). It opens the
  Keyboard shortcuts overview and drives terminal tabs.
- The desktop app's screenshot comparison, `python3 scripts/check-gui-goldens.py`, all scenes, locally (recorder run
  `23d6c397-625a-47fc-b031-86b61ee12c82`): passed.
- The docs website build, `bun install --frozen-lockfile && bun run build` in `website/`, locally: passed. Two docs
  pages changed (Keyboard shortcuts and Work in a session).
- `cargo fmt --all -- --check`, `python3 releasing/check-changelog.py format` and
  `python -B scripts/check-test-sleeps.py`, locally: passed. `dprint check`, locally: failed, on triage archive entries
  main gained from a review queue triage record (#4978) that this plan does not touch.

Reused: none. The executor's runs were focused selections, and everything they covered ran again above.

Skipped: the rest of the workspace Rust suite, since nothing outside the desktop GUI crate changed; a Mac run, though
the shortcut's macOS binding differs from Linux's (the tests cover both bindings' key positions on Linux); and CI, which
runs only on demand.

#### What the report says that is no longer true

- The report says the guards that keep the shortcut from acting behind a dialog, during input-method composition and
  with no usable terminal are covered by tests. Before the landing fix those tests could not fail on a missing guard
  (above).

#### Things you should know that the report does not say

- **From a tab beyond the 32 the app can show, both shortcuts do nothing.** The shortcut skips those tabs, but a user
  who clicks one sees a "not attached" notice, and from there both shortcuts do nothing until another tab is clicked. It
  is tested on purpose and SPEC.md's wording allows it, but a user would read it as a broken shortcut. It is rare; say
  if you want a TODO entry to let the shortcut step out of that tab.
- **Main fails `dprint check`.** Triage archive entries from the review queue triage record (#4978) are not wrapped as
  the Markdown formatter wants. This plan does not touch them; rewrapping them is a formatting-only PR. I recommend it;
  say if you want me to open it.
- **Small points.** The test for "only the main terminal" checks that the chord does not reach the program, but focus is
  on the tab strip there, so that check proves nothing (other tests cover it); one code comment line in the shortcut
  table was left unwrapped; and the new rows in the docs' Keyboard shortcuts table format the keys as code, while every
  other key on that page is plain text.
